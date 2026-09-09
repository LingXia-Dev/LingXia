export const supportedDisplayLanguages = ["en-US", "zh-CN"] as const;

export type DisplayLanguage = (typeof supportedDisplayLanguages)[number];

export type DisplayLanguagePreference = "auto" | DisplayLanguage;

export function resolveDisplayLanguage(language: unknown): DisplayLanguage {
  if (typeof language === "string" && language.toLowerCase().startsWith("zh")) {
    return "zh-CN";
  }
  return "en-US";
}

export function resolveDisplayLanguagePreference(
  preference: unknown,
): DisplayLanguagePreference {
  if (preference === "auto" || preference === "en-US" || preference === "zh-CN") {
    return preference;
  }
  return "auto";
}
