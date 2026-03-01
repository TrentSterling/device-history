export interface ThemeMeta {
  id: string;
  label: string;
  className: string;
}

export const themes: ThemeMeta[] = [
  { id: "neon", label: "Neon", className: "theme-neon" },
  { id: "dracula", label: "Dracula", className: "theme-dracula" },
  { id: "mocha", label: "Mocha", className: "theme-mocha" },
];

export function themeClass(id: string): string {
  return themes.find(t => t.id === id)?.className ?? "theme-neon";
}
