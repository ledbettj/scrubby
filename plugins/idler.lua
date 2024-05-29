local bot = require("bot")

local plugin = bot.plugin("Idler", "Scrubby is busy playing games.")

local last = 0
local games = {
   "HoboQuest II",
   "SimToilet Tycoon",
   "Microsoft GarbageTruck Simulator",
   "Minesweeper 2: Mine's Revenge",
   "Tom Clancy's The Janitorial"
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

bot:register(plugin)
