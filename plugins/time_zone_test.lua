local bot = require("bot")
local http = require("http")
local json = require("json")

local plugin = bot.plugin("Time Test")
local cache = plugin.cache

local zones = cache:get("zones")
if not zones then
   zones = {}
   print("fetching zones")
   local raw_zones = json.decode(http.get("https://www.timeapi.io/api/TimeZone/AvailableTimeZones"))
   for i, v in ipairs(raw_zones) do
      zones[v] = true
   end
   cache:set("zones", zones)
else
   print("using cached zones")
end

print(zones[0])

plugin:command("what time is it in ([^?]+)", function(msg, matches)
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
