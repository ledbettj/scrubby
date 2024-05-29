local bot = require("bot")
local http = require("http")
local json = require("json")

local plugin = bot.plugin("CatFact", "gives you facts. about cats.")

plugin:command("\\bcat fact", "gives you a fact.  about cats.", function(self, msg, _)
  local fact = json.decode(http.get("https://catfact.ninja/fact").body)
  return fact["fact"]
end)

bot:register(plugin)
