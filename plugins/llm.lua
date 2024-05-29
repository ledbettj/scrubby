local bot = require("bot")
local claude = require('claude')
local plugin = bot.plugin("LLM", "responds with generative AI from Claude.")

local CLAUDE_KEY = env.CLAUDE_KEY
local client = claude.Client:new(CLAUDE_KEY)

function plugin:fallback(msg)
   return client:respond(msg)
end

bot:register(plugin)
