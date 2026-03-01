export interface ThemeMeta {
  id: string;
  label: string;
  className: string;
}

export const themes: ThemeMeta[] = [
  { id: "neon", label: "Neon", className: "theme-neon" },
  { id: "mocha", label: "Mocha", className: "theme-mocha" },
  { id: "dracula", label: "Dracula", className: "theme-dracula" },
  { id: "nord", label: "Nord", className: "theme-nord" },
  { id: "solarized", label: "Solar", className: "theme-solarized" },
];

export function themeClass(id: string): string {
  return themes.find(t => t.id === id)?.className ?? "theme-neon";
}
