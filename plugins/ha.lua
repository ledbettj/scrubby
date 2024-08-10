local bot = require("bot")
local ha = require("homeassistant")
local json = require("json")
local plugin = bot.plugin("Home")

local client = ha.Client:new(env.HA_URL, env.HA_KEY)

local LIGHT_SCHEMA = {
   ["type"] = "object",
   properties = {
      target = {
         ["type"] = "string",
         description = "the name of the light to toggle. This must be an entity_id in the form 'light.foo_bar' from the output of the lights tool. "
      },
   },
   required = { "target" }
}


plugin:command({
      name = "light_toggle",
      description = [[
Toggle a light in the house (e.g. turn it on if it's off, and vice versa).  Do not perform this action unless explicitly asked to.
]],
      schema = LIGHT_SCHEMA,
      method = function(self, params)
         client:action('homeassistant', 'toggle', params.target)

         local r = client:state(params.target)
         return json.encode(r)
      end
})

plugin:command({
      name = "light_off",
      description = [[ Turn off a light in the house.  Do not perform this action unless explicitly asked to.]],
      schema = LIGHT_SCHEMA,
      method = function(self, params)
         client:action('homeassistant', 'turn_off', params.target)

         local r = client:state(params.target)
         return json.encode(r)
      end
})

plugin:command({
      name = "light_on",
      description = [[ Turn off a light in the house.  Do not perform this action unless explicitly asked to.]],
      schema = LIGHT_SCHEMA,
      method = function(self, params)
         client:action('homeassistant', 'turn_on', params.target)

         local r = client:state(params.target)
         return json.encode(r)
      end
})

plugin:command({
      name = "light_status",
      description = [[ Get the state of a light in the house. ]],
      schema = LIGHT_SCHEMA,
      method = function(self, params)
         local r = client:state(params.target)
         return json.encode(r)
      end
})


plugin:command({
      name = "light_list",
      description = [[ Get the list of lights that are usable in the house. ]],
      schema = nil,
      method = function(self, params)
         return json.encode({
               { entity_id = "light.bedroom", name = "Bedroom"},
               { entity_id = "light.porch", name = "Porch"},
               { entity_id = "light.landing", name = "Landing" },
               { entity_id = "light.entrance_hall", name = "Entrance Hall"},
               { entity_id = "light.tv_room", name = "TV Room"}
         })
      end
})

plugin:command({
      name = "camera_snap",
      description = [[ return a snapshot image from the camera ]],
      schema = nil,
      method = function(self, params)
         local r = client:state("camera.frontyard")

         return json.encode({
               instructions = "Format the response to the user as JSON with the included image",
               updated_at = r.last_updated,
               url = client.ha_url .. r.attributes.entity_picture
         })
      end
})


plugin:command({
      name = "hvac_status",
      description = [[ return the temperature and settings of the AC/Heat ]],
      schema = nil,
      method = function(self, params)
         local r = client:state("climate.nest")

         return json.encode({
               state = r.state,
               attrs = r.attributes
         })
      end
})


bot:register(plugin)
