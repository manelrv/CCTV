const pt = {
  title: "CCTV",

  state: {
    working: "Trabalhando",
    waiting_permission: "Aguardando permissão",
    waiting_input: "Aguardando entrada",
    idle: "Concluído",
    unknown: "Sem sinal",
    completed: "Concluído",
    error: "Erro",
  },

  summary: {
    instances_one: "{{count}} instância",
    instances_other: "{{count}} instâncias",
    attention_one: "{{count}} requer atenção",
    attention_other: "{{count}} requerem atenção",
  },

  copied: "Copiado",

  empty: "Sem instâncias ativas",
} as const;

export default pt;
