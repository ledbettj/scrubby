local bot = require("bot")
local plugin = bot.plugin("LLM")
local http = require('http')
local json = require('json')

local CLAUDE_KEY = env.CLAUDE_KEY
local CLAUDE_URL = "https://api.anthropic.com/v1/messages"
local CLAUDE_PROMPT = [[
You are a general purpose informational assistant.  Your name is Scrubby.
Your responses should be brief, informative, and limited to about the size of a tweet if possible.
Don't be afraid to be rude or sarcastic if necessary -- that's part of your charm.
]]

local history = {}

plugin:command("\\bforget\\b", function(self, msg, matches)
  history[msg.author] = nil
  return "OK"
end)

function plugin:fallback(msg)
   message = {
      ["role"] = "user",
      ["content"] = msg.content
   }
   history[msg.author] = history[msg.author] or {}
   table.insert(history[msg.author], message)

   local resp = http.post(
      CLAUDE_URL,
      json.encode({
            ["model"] = "claude-3-haiku-20240307",
            ["max_tokens"] = 1024,
            ["system"] = CLAUDE_PROMPT,
            ["messages"] = history[msg.author]
      }),
      {
         headers = {
            ["Content-Type"] = "application/json",
            ["X-API-Key"] = CLAUDE_KEY,
            ["Anthropic-Version"] = "2023-06-01"
         }
   })
   local body = json.decode(resp)
   table.insert(history[msg.author], { ["role"] = body.role, ["content"] = body.content[1].text })

   if #history[msg.author] >= 6 then
      -- have to remove two items (user/assistant)
      table.remove(history[msg.author], 1)
      table.remove(history[msg.author], 1)
   end

   return body.content[1].text
end

bot:register(plugin)
