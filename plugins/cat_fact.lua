local bot = require("bot")
local http = require("http")
local json = require("json")

local plugin = bot.plugin("CatFact")

plugin:command({
      name = "cat_fact",
      description = [[
Give an interesting or funny fact about cats of all types, for example: "Cheetahs can run more than 60 miles per hour." You must paraphrase this fact or return it verbatim when generating a response to the user.
]],
      schema = nil,
      method = function(self)
         local fact = json.decode(http.get("https://catfact.ninja/fact").body)
         return fact["fact"]
      end
})

bot:register(plugin)
