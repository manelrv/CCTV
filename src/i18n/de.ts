const de = {
  title: "CCTV",

  state: {
    working: "Arbeitet",
    waiting_permission: "Wartet auf Erlaubnis",
    waiting_input: "Wartet auf Eingabe",
    idle: "Fertig",
    unknown: "Kein Signal",
    completed: "Abgeschlossen",
    error: "Fehler",
  },

  summary: {
    instances_one: "{{count}} Instanz",
    instances_other: "{{count}} Instanzen",
    attention_one: "{{count}} erfordert Aufmerksamkeit",
    attention_other: "{{count}} erfordern Aufmerksamkeit",
  },

  copied: "Kopiert",

  empty: "Keine aktiven Instanzen",
} as const;

export default de;
