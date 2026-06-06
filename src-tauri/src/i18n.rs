//! Diccionario estático de cadenas UI para el menú de bandeja.
//! No se necesita lógica de plural en este lado (solo items de menú).

use sys_locale::get_locale;

/// Idiomas soportados. Cae a inglés si el locale del sistema no está en la lista.
#[derive(Clone, Copy)]
pub enum Lang {
    En,
    Es,
    Pt,
    De,
    Fr,
    It,
    Ca,
    Ru,
}

impl Lang {
    /// Detecta el idioma del sistema a partir del locale, descarta la región.
    pub fn detect() -> Self {
        let locale = get_locale().unwrap_or_default();
        // Toma solo el subtag de idioma (e.g. "es-ES" → "es").
        let lang = locale.split(['-', '_']).next().unwrap_or("").to_lowercase();
        match lang.as_str() {
            "es" => Lang::Es,
            "pt" => Lang::Pt,
            "de" => Lang::De,
            "fr" => Lang::Fr,
            "it" => Lang::It,
            "ca" => Lang::Ca,
            "ru" => Lang::Ru,
            _ => Lang::En,
        }
    }
}

pub struct TrayStrings {
    pub show_window: &'static str,
    pub floating_window: &'static str,
    pub always_on_top: &'static str,
    pub auto_hide: &'static str,
    pub compact_mode: &'static str,
    pub open_at_login: &'static str,
    pub quit: &'static str,
}

pub fn strings(lang: Lang) -> TrayStrings {
    match lang {
        Lang::Es => TrayStrings {
            show_window: "Mostrar ventana",
            floating_window: "Ventana flotante",
            always_on_top: "Siempre encima",
            auto_hide: "Auto-ocultar si nada me reclama",
            compact_mode: "Modo compacto",
            open_at_login: "Abrir al iniciar sesión",
            quit: "Salir",
        },
        Lang::Pt => TrayStrings {
            show_window: "Mostrar janela",
            floating_window: "Janela flutuante",
            always_on_top: "Sempre visível",
            auto_hide: "Auto-ocultar se nada precisar de mim",
            compact_mode: "Modo compacto",
            open_at_login: "Abrir ao iniciar sessão",
            quit: "Sair",
        },
        Lang::De => TrayStrings {
            show_window: "Fenster anzeigen",
            floating_window: "Schwebendes Fenster",
            always_on_top: "Immer im Vordergrund",
            auto_hide: "Automatisch ausblenden wenn nichts Aufmerksamkeit braucht",
            compact_mode: "Kompaktmodus",
            open_at_login: "Beim Anmelden öffnen",
            quit: "Beenden",
        },
        Lang::Fr => TrayStrings {
            show_window: "Afficher la fenêtre",
            floating_window: "Fenêtre flottante",
            always_on_top: "Toujours au premier plan",
            auto_hide: "Masquer automatiquement si rien ne nécessite attention",
            compact_mode: "Mode compact",
            open_at_login: "Ouvrir à la connexion",
            quit: "Quitter",
        },
        Lang::It => TrayStrings {
            show_window: "Mostra finestra",
            floating_window: "Finestra mobile",
            always_on_top: "Sempre in primo piano",
            auto_hide: "Nascondi automaticamente se nulla richiede attenzione",
            compact_mode: "Modalità compatta",
            open_at_login: "Apri all'accesso",
            quit: "Esci",
        },
        Lang::Ca => TrayStrings {
            show_window: "Mostra la finestra",
            floating_window: "Finestra flotant",
            always_on_top: "Sempre al damunt",
            auto_hide: "Amaga automàticament si res no em reclama",
            compact_mode: "Mode compacte",
            open_at_login: "Obre en iniciar sessió",
            quit: "Surt",
        },
        Lang::Ru => TrayStrings {
            show_window: "Показать окно",
            floating_window: "Плавающее окно",
            always_on_top: "Поверх всех окон",
            auto_hide: "Скрывать автоматически, если не требуется внимание",
            compact_mode: "Компактный режим",
            open_at_login: "Открывать при входе",
            quit: "Выйти",
        },
        // Inglés es el fallback.
        Lang::En => TrayStrings {
            show_window: "Show window",
            floating_window: "Floating window",
            always_on_top: "Always on top",
            auto_hide: "Auto-hide when nothing needs attention",
            compact_mode: "Compact mode",
            open_at_login: "Open at login",
            quit: "Quit",
        },
    }
}
