const fr = {
  title: "CCTV",

  state: {
    working: "En cours",
    waiting_permission: "En attente de permission",
    waiting_input: "En attente de saisie",
    idle: "Terminé",
    unknown: "Sans signal",
    completed: "Terminé",
    error: "Erreur",
  },

  summary: {
    instances_one: "{{count}} instance",
    instances_other: "{{count}} instances",
    attention_one: "{{count}} nécessite attention",
    attention_other: "{{count}} nécessitent attention",
  },

  empty: "Aucune instance active",
} as const;

export default fr;
