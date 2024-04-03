Plugin = Bot.plugin("Test Plugin 1.0")

function Plugin.on_message(msg)
   print(msg.author .. ": " .. msg.content)
   return "Takes one to know one"
end

Bot.register(Plugin)
