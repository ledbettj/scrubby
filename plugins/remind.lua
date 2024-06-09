local bot = require("bot")
local json = require("json")
local plugin = bot.plugin("Reminders")

plugin.cache:load()

plugin:command({
      name = "clear",
      description = [[Clear the list of reminders, removing all entries.]],
      schema = nil,
      method = function(self, params)
         self:log("Searching for ", json.encode(params))
         plugin.cache:clear()
         plugin.cache:save()
         return 'OK'
      end
})

plugin:command({
      name = "add",
      description = [[Add a short block of text to the list of reminders, so it can be retrieved later.]],
      schema = {
         ["type"] = "object",
         properties = {
            text = {
               ["type"] = "string",
               description = "The item to add to the reminders list."
            }
         },
         required = {"text"}
      },
      method = function(self, params)
         local text = params.text
         local list = self.cache:get("list") or { }
         table.insert(list, text)
         self.cache:set("list", list)
         self.cache:save()

         return "OK"
      end
})

plugin:command({
      name = "get",
      description = [[Get all the items that are in the reminder list.]],
      schema = nil,
      method = function(self, params)
         local list = self.cache:get("list") or { }

         return json.encode(list)
      end
})



bot:register(plugin)
