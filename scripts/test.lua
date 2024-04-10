local bot = require("bot")
local http = require("http")
local json = require("json")

local plugin = bot.plugin("test")

local _zones = json.decode(http.get("https://www.timeapi.io/api/TimeZone/AvailableTimeZones"))
local zones = {}
for i, v in ipairs(_zones) do
   zones[v] = true
   -- print(v)
end

bot:command("what time is it in ([^?]+)", function(msg, matches)
  local where = matches[2]
  local res = nil

  if zones[where] then
     local resp = json.decode(http.get("https://www.timeapi.io/api/TimeZone/zone?timeZone=" .. where))
     res = resp["currentLocalTime"]
  else
     res = 'Scrubby time'
  end

  return "It's " .. res .. " in " .. where .. ", " .. msg.author .. "."
end)

bot:register(plugin)
