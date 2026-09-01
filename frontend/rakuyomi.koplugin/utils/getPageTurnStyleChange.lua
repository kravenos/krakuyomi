---@class PageTurnStyleChange
---@field config_key string
---@field config_value number
---@field event_name string
---@field event_value boolean|string

--- Returns the KOReader configuration change needed for an explicit style.
--- @param configurable table|nil
--- @param style string|nil
--- @return PageTurnStyleChange|nil
return function(configurable, style)
  if configurable == nil then
    return nil
  end
  if style ~= "paginated" and style ~= "scroll" then
    return nil
  end

  local scroll = style == "scroll"
  local value = scroll and 1 or 0
  if configurable.page_scroll ~= nil then
    if configurable.page_scroll == value then
      return nil
    end
    return {
      config_key = "page_scroll",
      config_value = value,
      event_name = "SetScrollMode",
      event_value = scroll,
    }
  end

  if configurable.view_mode == value then
    return nil
  end
  return {
    config_key = "view_mode",
    config_value = value,
    event_name = "SetViewMode",
    event_value = scroll and "scroll" or "page",
  }
end
