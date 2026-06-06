// Ruso tiene 3 formas de plural:
// _one  → 1, 21, 31 ... (termina en 1, excepto 11)
// _few  → 2-4, 22-24 ... (termina en 2-4, excepto 12-14)
// _many → 0, 5-20, 11-14, 25-30 ... (el resto)
const ru = {
  title: "CCTV",

  state: {
    working: "Работает",
    waiting_permission: "Ожидает разрешения",
    waiting_input: "Ожидает ввода",
    idle: "Завершено",
    unknown: "Нет сигнала",
    completed: "Выполнено",
    error: "Ошибка",
  },

  summary: {
    instances_one: "{{count}} экземпляр",
    instances_few: "{{count}} экземпляра",
    instances_many: "{{count}} экземпляров",
    instances_other: "{{count}} экземпляра",
    attention_one: "{{count}} требует внимания",
    attention_few: "{{count}} требуют внимания",
    attention_many: "{{count}} требуют внимания",
    attention_other: "{{count}} требуют внимания",
  },

  empty: "Нет активных экземпляров",
} as const;

export default ru;
