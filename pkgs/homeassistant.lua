local http = require('http')
local json = require('json')

local Client = {}

function Client:new(ha_url, ha_key)
   client = { ha_url = ha_url, ha_key = ha_key }
   setmetatable(client, self)
   self.__index = self
   return client
end

function Client:action(service, domain, entity_id)
   local resp = http.post(
      self.ha_url .. "/api/services/" .. service .. "/" .. domain,
      json.encode({ entity_id = entity_id }),
      {
         json = true,
         headers = {
            ['Authorization'] = "Bearer " .. self.ha_key,
            ['Content-Type'] = 'application/json'
         }
      }
   )
   return resp.json
end


function Client:state(entity_id)
   local resp = http.get(
      self.ha_url .. "/api/states/" .. entity_id,
      {
         json = true,
         headers = {
            ['Authorization'] = "Bearer " .. self.ha_key
         }
      }
   )

   return resp.json
end


function Client:states()
   local resp = http.get(
      self.ha_url .. "/api/states",
      {
         json = true,
         headers = {
            ['Authorization'] = "Bearer " .. self.ha_key
         }
      }
   )

   return resp.json
end

return { Client = Client }
