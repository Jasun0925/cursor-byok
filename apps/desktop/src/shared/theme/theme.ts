export const themeIds = ["system", "default-dark", "default-light"] as const;
export type ThemeId = (typeof themeIds)[number];

export const themeOptions = [
  { id: "system" },
  { id: "default-dark" },
  { id: "default-light" },
] satisfies { id: ThemeId }[];

export function isThemeId(value: string | null): value is ThemeId {
  return value !== null && themeIds.some((id) => id === value);
}

let stopFollowingSystem: (() => void) | undefined;

function applyResolvedTheme(themeId: Exclude<ThemeId, "system">) {
  const light = themeId === "default-light";
  const background = light ? "#fcfcfc" : "#141414";
  const root = document.documentElement;
  root.dataset.theme = themeId;
  root.style.colorScheme = light ? "light" : "dark";
  root.style.setProperty("--startup-background", background);
  document.querySelector('meta[name="theme-color"]')?.setAttribute("content", background);
}

export function applyTheme(themeId: ThemeId) {
  stopFollowingSystem?.();
  stopFollowingSystem = undefined;

  if (themeId !== "system") {
    applyResolvedTheme(themeId);
    return;
  }

  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const update = () => applyResolvedTheme(media.matches ? "default-dark" : "default-light");
  update();
  media.addEventListener("change", update);
  stopFollowingSystem = () => media.removeEventListener("change", update);
}
