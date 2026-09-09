-- Run from the repository root: lua5.1 frontend/tests/reader_settings_test.lua
-- Read the real settings definitions and execute real preference helpers.
local stubs = {
  ["widgets/FocusManagerWithTopZone"] = { extend = function(__, value) return value end },
  ["gettext+"] = function(value) return value end,
  ["device"] = { screen = {}, isKindle = function() return true end },
  ["Paths"] = { getHomeDirectory = function() return "/test/rakuyomi" end },
  ["ffi"] = {
    cdef = function() end,
    new = function() return {} end,
    C = { sysinfo = function() return -1 end },
  },
}
for __, name in ipairs({
  "ffi/blitbuffer", "ui/widget/container/framecontainer", "ui/geometry",
  "ui/widget/horizontalgroup", "ui/widget/horizontalspan", "ui/widget/overlapgroup",
  "ui/size", "ui/widget/titlebar", "ui/uimanager", "ui/widget/verticalgroup",
  "ui/widget/infomessage", "ui/font", "ui/widget/textwidget",
  "ui/widget/container/scrollablecontainer", "ui/widget/container/movablecontainer",
  "Backend", "ErrorDialog", "widgets/SettingItem",
}) do stubs[name] = {} end
for name, value in pairs(stubs) do package.loaded[name] = value end
package.path = "frontend/rakuyomi.koplugin/?.lua;" .. package.path
local Settings = require("Settings")
local applyReadingDirection = require("utils/applyReadingDirection")
local getPageTurnStyleChange = require("utils/getPageTurnStyleChange")

local function definition_for(key)
  for __, tuple in ipairs(Settings.setting_value_definitions) do
    if tuple[1] == key then return tuple[2] end
  end
  error("missing setting: " .. key)
end
local failures = 0
local function test(name, run)
  local ok, message = pcall(run)
  if ok then print("PASS " .. name) else
    failures = failures + 1
    print("FAIL " .. name .. ": " .. tostring(message))
  end
end

for __, setting in ipairs({
  { key = "rakuyomi_reading_direction", choices = { "ltr", "rtl" } },
  { key = "rakuyomi_page_turn_style", choices = { "paginated", "scroll" } },
}) do
  test(setting.key .. " displays automatic when no preference is stored", function()
    local definition = definition_for(setting.key)
    assert(definition.is_local and definition.type == "enum")
    assert(definition.default == "auto", "unset preference is displayed as an explicit choice")
    -- The enum widget also uses its first option when passed a nil value.
    assert(definition.options[1].value == "auto", "nil enum fallback does not follow viewer mode")
    assert(type(definition.options[1].label) == "string" and #definition.options[1].label > 0)
  end)

  test(setting.key .. " retains every explicit choice", function()
    local options = {}
    for __, option in ipairs(definition_for(setting.key).options) do options[option.value] = true end
    for __, choice in ipairs(setting.choices) do assert(options[choice], "missing choice: " .. choice) end
  end)
end

test("automatic and unset direction preserve both existing viewer directions", function()
  for __, inverse in ipairs({ false, true }) do
    local view = {
      inverse_reading_order = inverse,
      onToggleReadingOrder = function() error("automatic direction must not override the viewer") end,
    }
    assert(applyReadingDirection(view, "auto") == false)
    assert(applyReadingDirection(view, nil) == false)
    assert(view.inverse_reading_order == inverse)
  end
end)

test("automatic and unset page style preserve existing scrolling or pagination", function()
  for __, configurable in ipairs({ { page_scroll = 0 }, { page_scroll = 1 }, { view_mode = 0 }, { view_mode = 1 } }) do
    assert(getPageTurnStyleChange(configurable, "auto") == nil)
    assert(getPageTurnStyleChange(configurable, nil) == nil)
  end
end)

assert(failures == 0, tostring(failures) .. " reader settings tests failed")
