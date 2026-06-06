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

// Detecta el idioma del navegador, descarta la región (e.g. "es-ES" → "es").
// Cae a "en" si el idioma no está soportado.
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
    // React ya escapa por defecto; no hace falta doble escape.
    escapeValue: false,
  },
});

export default i18next;
