import i18next from "i18next";
import { initReactI18next } from "react-i18next";

import en from "./en";
import es from "./es";
import pt from "./pt";
import de from "./de";
import fr from "./fr";
import it from "./it";
import ca from "./ca";
import ru from "./ru";

// Detects the browser language and strips the region tag (e.g. "es-ES" → "es").
// Falls back to "en" if the language is not supported.
const SUPPORTED = ["en", "es", "pt", "de", "fr", "it", "ca", "ru"] as const;

function detectLanguage(): string {
  const lang = navigator.language.split("-")[0].toLowerCase();
  return (SUPPORTED as readonly string[]).includes(lang) ? lang : "en";
}

i18next.use(initReactI18next).init({
  lng: detectLanguage(),
  fallbackLng: "en",
  resources: {
    en: { translation: en },
    es: { translation: es },
    pt: { translation: pt },
    de: { translation: de },
    fr: { translation: fr },
    it: { translation: it },
    ca: { translation: ca },
    ru: { translation: ru },
  },
  interpolation: {
    // React escapes by default; no need for double escaping.
    escapeValue: false,
  },
});

export default i18next;
