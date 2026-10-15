<script lang="ts">
  // Import Flashcards modal — the paste side of the flashcard import flow.
  // Lets the user pick a target TOPIC (required — nothing may land untopiced),
  // a deck title, and a FORMAT, then paste content. Parsing/dedupe/persistence
  // live in the Rust `import_flashcards` command (see flashparse.rs); this is a
  // thin themed shell built from the design system: .input, .seg/.seg-opt, .btn.
  import * as api from "../lib/api";
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";

  let { onClose, onImported }: { onClose: () => void; onImported: () => void } = $props();

  // One entry per supported format. `id` is the stable key the backend parser
  // switches on; `hint` documents the paste shape; `sample` seeds the box so the
  // user sees the exact shape expected before typing.
  const FORMATS: { id: string; label: string; hint: string; sample: string; file?: boolean }[] = [
    {
      id: "apkg",
      label: "Anki .apkg",
      hint: "Import an Anki .apkg archive (one deck per Anki deck). Pick the file below.",
      sample: "",
      file: true,
    },
    {
      id: "anki_json",
      label: "Anki JSON",
      hint: 'A JSON array of cards — front/back, q/a, term/definition, word/translation, or ["f","b"] pairs.',
      sample: '[\n  { "front": "What is RAM?", "back": "Random Access Memory." },\n  { "front": "What is ROM?", "back": "Read-Only Memory." }\n]',
    },
    {
      id: "quizlet",
      label: "Quizlet",
      hint: "Two columns — tab between term and definition (native Quizlet export), or term on one line and definition on the next.",
      sample: "What is a computer cable?\tA cable transmits power or data between devices.\nWhat are the two main types of cables?\tData cables and power cables.",
    },
    {
      id: "memrise",
      label: "Memrise",
      hint: "Word then translation — tab-separated, or the word on one line and translation on the next.",
      sample: "hola\thello\nadiós\tgoodbye\npor favor\tplease",
    },
    {
      id: "csv",
      label: "CSV",
      hint: "Two columns per row — front,back. Quote a field if it contains a comma. A header row (front,back / term,definition) is ignored.",
      sample: 'What is a computer cable?,A cable transmits power or data between devices.\n"According to the 20-20-20 Rule, how long should you look at something 20 feet away?",At least 20 seconds.',
    },
    {
      id: "newline",
      label: "Term / Definition",
      hint: 'One "Term:" line + one "Definition:" line per card, separated by a blank line.',
      sample: "Term: What is a computer cable?\nDefinition: A cable transmits power or data between devices.\n\nTerm: What are the two main types of cables?\nDefinition: Data cables and power cables.",
    },
    {
      id: "tab",
      label: "Tab-separated",
      hint: "Term and definition on one line, a tab between them, a new line between cards.",
      sample: "What is a computer cable?\tA cable transmits power or data between devices.\nWhat are the two main types of cables?\tData cables and power cables.",
    },
    {
      id: "colon",
      label: "Colon-separated",
      hint: "Simplest — the first colon separates term from definition, one card per line.",
      sample: "What is a computer cable?: A cable transmits power or data between devices.\nWhat are the two main types of cables?: Data cables and power cables.",
    },
    {
      id: "qa",
      label: "Q / A",
      hint: 'One "Q:" line + one "A:" line per card, separated by a blank line.',
      sample: "Q: What is a computer cable?\nA: A cable transmits power or data between devices.\n\nQ: What are the two main types of cables?\nA: Data cables and power cables.",
    },
  ];

  const subj = $derived(app.activeSubject);
  const topics = $derived(subj?.topics ?? []);
  const curFormat = $derived(FORMATS.find((f) => f.id === format) ?? FORMATS[0]);

  let format = $state("newline");
  let title = $state("");
  let text = $state(FORMATS.find((f) => f.id === "newline")?.sample ?? "");
  let apkgPath = $state("");
  let topicId = $state("");
  let busy = $state(false);

  // Default the topic to the first available one once topics load.
  $effect(() => {
    if (!topicId && topics.length > 0) topicId = topics[0].id;
  });

  // Swap the sample whenever the chosen format changes on a button click.
  // A .apkg import is file-based, so it has no sample — clear the paste box.
  function setFormat(id: string) {
    format = id;
    apkgPath = "";
    const f = FORMATS.find((x) => x.id === id);
    text = f?.sample ?? "";
  }

  // Pick an Anki .apkg through the native open dialog (only shown for apkg).
  async function pickApkg() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const picked = await open({ multiple: false, filters: [{ name: "Anki deck", extensions: ["apkg"] }] });
      const path = Array.isArray(picked) ? picked[0] : picked;
      if (path) apkgPath = path;
    } catch (e) {
      app.pushToast({ kind: "error", title: "Couldn't open file picker", body: String(e) });
    }
  }
  async function createTopic() {
    const name = await app.prompt({
      title: `New topic in ${subj?.name ?? "subject"}`,
      label: "Topic name",
      placeholder: "e.g. Computer Basics",
    });
    if (!name?.trim()) return;
    try {
      await app.createTopic(name.trim());
      // createTopic refreshes the store; explicitly select the just-created topic.
      const created = app.activeSubject?.topics.find((t) => t.name === name.trim());
      if (created) topicId = created.id;
    } catch (e) {
      app.pushToast({ kind: "error", title: "Couldn't create topic", body: String(e) });
    }
  }

  async function runImport() {
    if (!subj) { app.pushToast({ kind: "warning", title: "Select a subject first" }); return; }
    if (!topicId) { app.pushToast({ kind: "warning", title: "Pick a topic first" }); return; }
    if (curFormat.file) {
      if (!apkgPath) { app.pushToast({ kind: "warning", title: "Choose an .apkg file first" }); return; }
      busy = true;
      try {
        const r = await api.importAnki(subj.id, apkgPath, topicId);
        const decks = `${r.deck_count} deck${r.deck_count !== 1 ? "s" : ""}`;
        const cards = `${r.card_count} card${r.card_count !== 1 ? "s" : ""}`;
        app.pushToast(
          r.skipped > 0
            ? { kind: "warning", title: "Imported with skips", body: `${cards} across ${decks} · ${r.skipped} duplicate${r.skipped !== 1 ? "s" : ""} skipped` }
            : { kind: "success", title: "Imported from Anki", body: `${cards} across ${decks}` }
        );
        onImported();
        onClose();
      } catch (err) {
        app.pushToast({ kind: "error", title: "Import failed", body: String(err) });
      } finally {
        busy = false;
      }
      return;
    }
    if (!text.trim()) { app.pushToast({ kind: "warning", title: "Paste some flashcards first" }); return; }
    busy = true;
    try {
      const r = await api.importFlashcards(subj.id, topicId, title.trim(), format, text);
      const cards = `${r.card_count} card${r.card_count !== 1 ? "s" : ""}`;
      app.pushToast(
        r.skipped > 0
          ? { kind: "warning", title: "Imported with skips", body: `${cards} · ${r.skipped} duplicate${r.skipped !== 1 ? "s" : ""} skipped` }
          : { kind: "success", title: "Imported flashcards", body: cards }
      );
      onImported();
      onClose();
    } catch (err) {
      app.pushToast({ kind: "error", title: "Import failed", body: String(err) });
    } finally {
      busy = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") { e.preventDefault(); onClose(); }
  }
</script>


<svelte:window onkeydown={onKey} />
<div class="imp-back" role="presentation" onmousedown={() => onClose()}>
  <div class="imp" role="dialog" aria-modal="true" aria-label="Import flashcards" tabindex="-1" onmousedown={(e) => e.stopPropagation()}>
    <div class="imp-head">
      <div class="imp-title"><Icon name="upload" size={14} /> Import flashcards</div>
      <button class="btn btn--ghost btn--icon btn--sm" type="button" aria-label="Close" onclick={() => onClose()}>
        <Icon name="x" size={13} />
      </button>
    </div>

    <!-- TOPIC (required) — anything imported must belong to a topic. -->
    <div class="imp-row">
      <div class="imp-lbl">Topic <span class="imp-req">required</span></div>
      <div class="imp-topic">
        <select class="input imp-select" bind:value={topicId} disabled={topics.length === 0}>
          {#if topics.length === 0}
            <option value="">No topics yet — create one</option>
          {:else}
            {#each topics as t (t.id)}
              <option value={t.id}>{t.name}</option>
            {/each}
          {/if}
        </select>
        <button class="btn btn--sm btn--ghost" type="button" onclick={createTopic} title="Create a new topic">
          <Icon name="plus" size={12} /> New topic
        </button>
      </div>
    </div>

    <!-- Deck title (optional — backend falls back to "Imported flashcards"). -->
    <div class="imp-row">
      <div class="imp-lbl">Deck title <span class="imp-opt">optional</span></div>
      <input class="input" bind:value={title} placeholder="e.g. Computer Basics" />
    </div>

    <!-- FORMAT picker -->
    <div class="imp-row">
      <div class="imp-lbl">Format</div>
      <div class="seg imp-formats" role="group" aria-label="Import format">
        {#each FORMATS as f (f.id)}
          <button type="button" class={"seg-opt" + (format === f.id ? " on" : "")} onclick={() => setFormat(f.id)}>
            {f.label}
          </button>
        {/each}
      </div>
      <div class="imp-hint">{curFormat.hint}</div>
    </div>

    <!-- CONTENT: a file picker for .apkg, else a paste box -->
    <div class="imp-row">
      <div class="imp-lbl">{curFormat.file ? "File" : "Content"}</div>
      {#if curFormat.file}
        <div class="imp-file">
          <button class="btn btn--sm" type="button" onclick={pickApkg}>
            <Icon name="upload" size={12} /> Choose .apkg…
          </button>
          <span class="imp-filepath mono">{apkgPath ? apkgPath.split(/[\\/]/).pop() : "No file chosen"}</span>
        </div>
      {:else}
        <textarea class="input imp-textarea" bind:value={text} rows="10" spellcheck="false" placeholder="Paste your flashcards here…"></textarea>
      {/if}
    </div>

    <div class="imp-foot">
      <button class="btn btn--ghost btn--sm" type="button" onclick={() => onClose()}>Cancel</button>
      <button class="btn btn--primary btn--sm" type="button" onclick={runImport} disabled={busy}>
        <Icon name="upload" size={12} /> {busy ? "Importing…" : "Import"}
      </button>
    </div>
  </div>
</div>

<style>
  .imp-back {
    position: fixed; inset: 0; z-index: 200;
    display: flex; align-items: center; justify-content: center;
    background: color-mix(in oklab, var(--bg) 62%, transparent);
    backdrop-filter: blur(3px);
    animation: imp-fade 0.12s ease;
  }
  .imp {
    width: min(560px, calc(100vw - 40px));
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--rad-4);
    box-shadow: var(--shadow-pop);
    padding: 18px;
    animation: imp-pop 0.13s ease;
  }
  @keyframes imp-fade { from { opacity: 0; } to { opacity: 1; } }
  @keyframes imp-pop { from { opacity: 0; transform: translateY(6px) scale(0.99); } to { opacity: 1; transform: none; } }
  .imp-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 14px; }
  .imp-title { display: flex; align-items: center; gap: 8px; font-family: var(--font-mono); font-size: var(--t-md); font-weight: 600; color: var(--fg-bright); }
  .imp-row { margin-top: 12px; }
  .imp-lbl { display: block; margin-bottom: 6px; font-size: var(--t-2xs); font-weight: 600; letter-spacing: 0.12em; text-transform: uppercase; color: var(--fg-faint); }
  .imp-req { color: var(--warn); letter-spacing: 0; text-transform: none; font-weight: 600; }
  .imp-opt { color: var(--fg-faint); letter-spacing: 0; text-transform: none; font-weight: 400; opacity: 0.7; }
  .imp-topic { display: flex; gap: 8px; align-items: center; }
  .imp-file { display: flex; gap: 10px; align-items: center; }
  .imp-filepath { flex: 1; min-width: 0; font-size: var(--t-xs); color: var(--fg-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .imp-select { flex: 1; min-width: 0; }
  .imp-formats { flex-wrap: wrap; row-gap: 4px; }
  .imp-hint { margin-top: 7px; font-size: var(--t-xs); color: var(--fg-muted); line-height: 1.45; }
  .imp-textarea { width: 100%; resize: vertical; font-family: var(--font-mono); font-size: var(--t-xs); line-height: 1.5; min-height: 150px; white-space: pre; }
  .imp-foot { display: flex; justify-content: flex-end; gap: 8px; margin-top: 16px; }
</style>

