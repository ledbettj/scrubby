local Plugin = Bot.plugin("Test")

function Plugin.on_message(msg)
   print("[" .. Bot.name .. "] " .. msg.author .. ": " .. msg.content)
   return "Takes one to know one, " .. msg.author
end

Bot.register(Plugin)
