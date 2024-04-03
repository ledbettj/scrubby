local Plugin = Bot.plugin("Test")

Bot.command("what time is it in ([^?]+)", function(msg, matches)
  return "It's Scrubby time in " .. matches[2] .. ", " .. msg.author .. '.'
end)

Bot.register(Plugin)
