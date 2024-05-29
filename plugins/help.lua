local bot = require("bot")
local plugin = bot.plugin("help", "provides commands that describe the supported plugins and their commands.")

plugin:command("list\\s+plugins", "lists all available loaded plugins.", function(self, msg, matches)
  local names = {"**Plugins**" }
  for name, obj in pairs(bot.plugins) do
     table.insert(names, "* " .. name .. " - " .. (obj.description or "(no description)"))
  end

  return table.concat(names, "\n") .. "\nAsk `about <plugin>` for details."
end)

plugin:command("about\\s+([^\\s]+)", "describe the commands and functionality of a given plugin.", function(self, msg, matches)
  local query = string.lower(matches[2])

  for name, obj in pairs(bot.plugins) do
     if string.lower(name) == query then
        local cmds = { "**" .. name .. " **", (obj.description or "(no description)") }

        if obj.fallback then
           table.insert(cmds, "This plugin responds to all non-matched input.")
        end

        if next(obj.commands) == nil then
           table.insert(cmds, "_(no commands provided)_")
        else
           table.insert(cmds, "Commands:")
        end

        for cmd, _ in pairs(obj.commands) do
           table.insert(cmds, "* `" .. cmd .. "`" .. " - " .. (obj.help[cmd] or "_(no description provided)_"))
        end

        return table.concat(cmds, "\n")
     end
  end

  return "Sorry, don't know anything about that plugin."
end)

bot:register(plugin)
