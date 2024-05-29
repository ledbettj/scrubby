local http = require('http')
local json = require('json')
local bot = require('bot')

local CLAUDE_URL = "https://api.anthropic.com/v1/messages"
local CLAUDE_PROMPT = [[
Respond to all messages in JSON format. You should respond with one of these two object formats:
{ "action": "message", "content": "text to return to the user" }

Or if you think you can answer the user's question by invoking a tool, return:
{ "action": "tool", "tool": "tool name", "parameters": { "name1": "value1", "name2": "value2" } }

The following tools are available:

Tool Name: "CatFact"
Tool description: Returns an interesting or useful fact about cats
Tool Parameters: none

When responding with action: "message", remember, you are a general purpose informational assistant.
Your name is Scrubby.
Your message content should be brief, informative, and limited to about the size of a tweet if possible.
Don't be afraid to be rude or sarcastic if necessary -- that's part of your charm.
]]

local Client = {}

function Client:new(api_key, system_prompt)
   client = {
      api_key = api_key,
      system_prompt = (system_prompt or CLAUDE_PROMPT),
      history = {}
   }
   setmetatable(client, self)
   self.__index = self
   return client
end

function Client:request(author, msg)
   self.history[author] = self.history[author] or {}
   table.insert(self.history[author], { role = msg.role, content = msg.content })

   local resp = http.post(
      CLAUDE_URL,
      json.encode({
            model = "claude-3-haiku-20240307",
            max_tokens = 1024,
            system = CLAUDE_PROMPT,
            messages = self.history[author]
      }),
      {
         json = true,
         headers = {
            ["Content-Type"] = "application/json",
            ["X-API-Key"] = self.api_key,
            ["Anthropic-Version"] = "2023-06-01"
         }
   })

   return resp
end

function Client:respond(msg)
   local author = msg.author
   msg = { role = "user", content = msg.content, author = msg.author }

   while true do
      local resp = self:request(author, msg)
      print(resp.body)
      if resp.status ~= 200 or resp.json.type == "error" then
         return "Error:\n```\n" .. resp.body .. "\n```"
      end

      table.insert(self.history[author], { role = "assistant", content = json.encode(payload) })

      local payload = json.decode(resp.json.content[1].text)
      print(json.encode(payload))

      if payload.action == "tool" then
         local tool = bot.plugins[payload.tool]
         local reply = tool.commands[payload.tool](tool, nil, nil)
         msg = { role = "user", content = json.encode({ tool = payload.tool, output = reply }) }
      elseif payload.action == "message" then
         return payload.content
      end
   end
end

return { Client = Client }
