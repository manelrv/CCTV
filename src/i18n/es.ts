const es = {
  title: "CCTV",

  state: {
    working: "Trabajando",
    waiting_permission: "Esperando permiso",
    waiting_input: "Esperando input",
    idle: "Terminado",
    unknown: "Sin señal",
    completed: "Completado",
    error: "Error",
  },

  summary: {
    instances_one: "{{count}} instancia",
    instances_other: "{{count}} instancias",
    attention_one: "{{count}} requiere atención",
    attention_other: "{{count}} requieren atención",
  },

  copied: "Copiado",

  empty: "Sin instancias activas",
} as const;

export default es;
