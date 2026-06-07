const ca = {
  title: "CCTV",

  state: {
    working: "Treballant",
    waiting_permission: "Esperant permís",
    waiting_input: "Esperant entrada",
    idle: "Acabat",
    unknown: "Sense senyal",
    completed: "Completat",
    error: "Error",
  },

  summary: {
    instances_one: "{{count}} instància",
    instances_other: "{{count}} instàncies",
    attention_one: "{{count}} requereix atenció",
    attention_other: "{{count}} requereixen atenció",
  },

  copied: "Copiat",

  empty: "Sense instàncies actives",
} as const;

export default ca;
