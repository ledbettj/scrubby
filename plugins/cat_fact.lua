local bot = require("bot")
local http = require("http")
local json = require("json")

local plugin = bot.plugin("Cat Facts")

plugin:command(".*cat fact", function(msg, _)
  local fact = json.decode(http.get("https://catfact.ninja/fact"))
  return fact["fact"]
end)

bot:register(plugin)
