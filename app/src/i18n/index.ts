// Internationalisation. To add a language: copy en.json, translate it, and
// add it to `languages` below. English is the fallback for missing keys.

import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import en from "./en.json";
import es from "./es.json";

export const languages = [
  { code: "en", label: "English" },
  { code: "es", label: "Español" },
] as const;

void i18n.use(initReactI18next).init({
  resources: { en: { translation: en }, es: { translation: es } },
  lng: "en",
  fallbackLng: "en",
  interpolation: { escapeValue: false },
});

export default i18n;
