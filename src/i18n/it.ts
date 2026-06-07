const it = {
  title: "CCTV",

  state: {
    working: "In esecuzione",
    waiting_permission: "In attesa di permesso",
    waiting_input: "In attesa di input",
    idle: "Terminato",
    unknown: "Nessun segnale",
    completed: "Completato",
    error: "Errore",
  },

  summary: {
    instances_one: "{{count}} istanza",
    instances_other: "{{count}} istanze",
    attention_one: "{{count}} richiede attenzione",
    attention_other: "{{count}} richiedono attenzione",
  },

  copied: "Copiato",

  empty: "Nessuna istanza attiva",
} as const;

export default it;
