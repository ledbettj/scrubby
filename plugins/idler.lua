local bot = require("bot")

local plugin = bot.plugin("Idler")

local last = 0
local games = {
   "HoboQuest II",
   "SimToilet Tycoon",
   "Microsoft GarbageTruck Simulator",
   "Minesweeper 2: Mine's Revenge",
   "Tom Clancy's The Janitorial"
}

local fallbacks = {
   "Really?",
    "That's a head-scratcher.",
    "Well, that's a new one.",
    "Are you serious?",
    "It's a bold strategy, Cotton.",
    "You're full of surprises.",
    "Fascinating question.",
    "You win the creativity award.",
    "Now that's a brain-teaser.",
    "That's one way to put it.",
    "I see what you did there.",
    "You're not holding back, are you?",
    "Well, that's original.",
    "I'll give you points for originality.",
    "Well, that's a first."
}
function plugin:tick(ctx)
   if os.time() < last + 60 * 5 then
      return
   end
   local game = games[math.random(1, #games)]
   ctx:set_activity(game)
   self:log("activity set to ", game)
   last = os.time()
end

function plugin:ready(ctx)
   ctx:idle()
end

function plugin:fallback(msg)
   return fallbacks[math.random(1, #fallbacks)]
end

bot:register(plugin)
