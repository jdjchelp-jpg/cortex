export type Locale = "en" | "es-419" | "fr" | "zh-CN" | "nl" | "de" | "pt-BR" | "ja" | "ko" | "ar";

export const LOCALES: { id: Locale; label: string; native: string }[] = [
  { id: "en", label: "English", native: "English" },
  { id: "es-419", label: "Simplified Spanish", native: "Español (Latinoamérica)" },
  { id: "fr", label: "French", native: "Français" },
  { id: "zh-CN", label: "Simplified Chinese", native: "简体中文" },
  { id: "nl", label: "Dutch", native: "Nederlands" },
  { id: "de", label: "German", native: "Deutsch" },
  { id: "pt-BR", label: "Portuguese (Brazil)", native: "Português (Brasil)" },
  { id: "ja", label: "Japanese", native: "日本語" },
  { id: "ko", label: "Korean", native: "한국어" },
  { id: "ar", label: "Arabic", native: "العربية" },
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
  fr: { Profile: "Profil", Identity: "Identité", "Display name": "Nom affiché", Pronouns: "Pronoms", Level: "Niveau", "Field of study": "Domaine d’étude", "About you": "À propos de vous", Memory: "Mémoire", "Save profile": "Enregistrer le profil", Remember: "Mémoriser", "Google Calendar": "Google Agenda", "Sync now": "Synchroniser", Calendar: "Agenda", References: "Références", Assignments: "Devoirs", "Copy all": "Tout copier", Import: "Importer", Export: "Exporter", New: "Nouveau", "Study music": "Musique d’étude", Stations: "Stations", "Your stations": "Vos stations", Favourites: "Favoris" },
  "zh-CN": { Profile: "个人资料", Identity: "身份", "Display name": "显示名称", Pronouns: "代词", Level: "学习阶段", "Field of study": "学习领域", "About you": "关于你", Memory: "记忆", "Save profile": "保存资料", Remember: "记住", "Google Calendar": "Google 日历", "Sync now": "立即同步", Calendar: "日历", References: "参考文献", Assignments: "作业", "Copy all": "全部复制", Import: "导入", Export: "导出", New: "新建", "Study music": "学习音乐", Stations: "电台", "Your stations": "你的电台", Favourites: "收藏" },
  nl: { Profile: "Profiel", Identity: "Identiteit", "Display name": "Weergavenaam", Pronouns: "Voornaamwoorden", Level: "Niveau", "Field of study": "Studiegebied", "About you": "Over jou", Memory: "Geheugen", "Save profile": "Profiel opslaan", Remember: "Onthouden", "Google Calendar": "Google Agenda", "Sync now": "Nu synchroniseren", Calendar: "Agenda", References: "Referenties", Assignments: "Opdrachten", "Copy all": "Alles kopiëren", Import: "Importeren", Export: "Exporteren", New: "Nieuw", "Study music": "Studiemuziek", Stations: "Stations", "Your stations": "Jouw stations", Favourites: "Favorieten" },
  de: { Profile: "Profil", Identity: "Identität", "Display name": "Anzeigename", Pronouns: "Pronomen", Level: "Niveau", "Field of study": "Studienfach", "About you": "Über dich", Memory: "Erinnerungen", "Save profile": "Profil speichern", Remember: "Merken", "Google Calendar": "Google Kalender", "Sync now": "Jetzt synchronisieren", Calendar: "Kalender", References: "Quellen", Assignments: "Aufgaben", "Copy all": "Alles kopieren", Import: "Importieren", Export: "Exportieren", New: "Neu", "Study music": "Lernmusik", Stations: "Sender", "Your stations": "Deine Sender", Favourites: "Favoriten" },
  "pt-BR": { Profile: "Perfil", Identity: "Identidade", "Display name": "Nome de exibição", Pronouns: "Pronomes", Level: "Nível", "Field of study": "Área de estudo", "About you": "Sobre você", Memory: "Memória", "Save profile": "Salvar perfil", Remember: "Lembrar", "Google Calendar": "Google Agenda", "Sync now": "Sincronizar agora", Calendar: "Calendário", References: "Referências", Assignments: "Tarefas", "Copy all": "Copiar tudo", Import: "Importar", Export: "Exportar", New: "Novo", "Study music": "Música de estudo", Stations: "Estações", "Your stations": "Suas estações", Favourites: "Favoritos" },
  ja: { Profile: "プロフィール", Identity: "基本情報", "Display name": "表示名", Pronouns: "代名詞", Level: "レベル", "Field of study": "専攻", "About you": "あなたについて", Memory: "メモリー", "Save profile": "プロフィールを保存", Remember: "記憶する", "Google Calendar": "Google カレンダー", "Sync now": "今すぐ同期", Calendar: "カレンダー", References: "参考文献", Assignments: "課題", "Copy all": "すべてコピー", Import: "インポート", Export: "エクスポート", New: "新規", "Study music": "学習用音楽", Stations: "ステーション", "Your stations": "自分のステーション", Favourites: "お気に入り" },
  ko: { Profile: "프로필", Identity: "신원", "Display name": "표시 이름", Pronouns: "대명사", Level: "수준", "Field of study": "전공", "About you": "나에 대해", Memory: "메모리", "Save profile": "프로필 저장", Remember: "기억하기", "Google Calendar": "Google 캘린더", "Sync now": "지금 동기화", Calendar: "캘린더", References: "참고문헌", Assignments: "과제", "Copy all": "모두 복사", Import: "가져오기", Export: "내보내기", New: "새로 만들기", "Study music": "학습 음악", Stations: "스테이션", "Your stations": "내 스테이션", Favourites: "즐겨찾기" },
  ar: { Profile: "الملف الشخصي", Identity: "الهوية", "Display name": "اسم العرض", Pronouns: "الضمائر", Level: "المستوى", "Field of study": "مجال الدراسة", "About you": "عن نفسك", Memory: "الذاكرة", "Save profile": "حفظ الملف الشخصي", Remember: "تذكّر", "Google Calendar": "تقويم Google", "Sync now": "مزامنة الآن", Calendar: "التقويم", References: "المراجع", Assignments: "المهام", "Copy all": "نسخ الكل", Import: "استيراد", Export: "تصدير", New: "جديد", "Study music": "موسيقى الدراسة", Stations: "المحطات", "Your stations": "محطاتك", Favourites: "المفضلة" },
};

export function translate(locale: Locale, text: string): string {
  return strings[locale][text] ?? text;
}
