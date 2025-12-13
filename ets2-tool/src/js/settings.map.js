export const SETTINGS_MAP = {
  money: {
    key: "info_money_account",
    file: "save",
    type: "number",
    reload: ["profile"]
  },

  xp: {
    key: "info_players_experience",
    file: "save",
    type: "number",
    reload: ["profile"]
  },

  level: {
    key: "info_player_level",
    file: "save",
    type: "number",
    reload: ["profile"]
  },

  convoy_size: {
    key: "max_convoy_size",
    file: "config",
    type: "number",
    reload: ["baseConfig"]
  },

  traffic: {
    key: "traffic",
    file: "config",
    type: "number",
    reload: ["baseConfig"]
  },

  developer: {
    key: "developer",
    file: "config",
    type: "bool",
    reload: ["baseConfig"]
  },

  console: {
    key: "console",
    file: "config",
    type: "bool",
    reload: ["baseConfig"]
  },

  parking_doubles: {
    key: "factor_parking_doubles",
    file: "game",
    type: "bool",
    reload: ["saveConfig"]
  },

  skill_adr: {
    key: "adr",
    file: "save",
    type: "adr",
    reload: ["quicksave"]
  },

  skill_long: {
    key: "long_dist",
    file: "save",
    type: "number",
    reload: ["quicksave"]
  },

  skill_heavy: {
    key: "heavy",
    file: "save",
    type: "number",
    reload: ["quicksave"]
  },

  skill_fragile: {
    key: "fragile",
    file: "save",
    type: "number",
    reload: ["quicksave"]
  },

  skill_urgent: {
    key: "urgent",
    file: "save",
    type: "number",
    reload: ["quicksave"]
  },

  skill_eco: {
    key: "mechanical",
    file: "save",
    type: "number",
    reload: ["quicksave"]
  }
};
