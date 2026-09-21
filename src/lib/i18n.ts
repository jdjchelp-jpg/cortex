export type Locale = "en" | "es-419";

export const LOCALES: { id: Locale; label: string; native: string }[] = [
  { id: "en", label: "English", native: "English" },
  { id: "es-419", label: "Simplified Spanish", native: "Español (Latinoamérica)" },
];

const strings: Record<Locale, Record<string, string>> = {
  en: {},
  "es-419": {
    Profile: "Perfil", Identity: "Identidad", "Display name": "Nombre para mostrar", Pronouns: "Pronombres",
    "Level": "Nivel", "Field of study": "Área de estudio", "About you": "Sobre ti", Memory: "Memoria",
    "Save profile": "Guardar perfil", Remember: "Recordar", "No memories yet. Add a fact above and the AI will keep it in mind.": "Aún no hay recuerdos. Añade un dato y la IA lo tendrá en cuenta.",
    "Google Calendar": "Calendario de Google", "Sync now": "Sincronizar ahora", Calendar: "Calendario", References: "Referencias",
    Assignments: "Tareas", "Copy all": "Copiar todo", Import: "Importar", Export: "Exportar", New: "Nuevo",
    "Study music": "Música de estudio", Stations: "Estaciones", "Your stations": "Tus estaciones", Favourites: "Favoritos",
  },
};

export function translate(locale: Locale, text: string): string {
  return strings[locale][text] ?? text;
}
