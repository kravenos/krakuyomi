-- Run from the repository root: lua5.1 frontend/tests/source_status_test.lua
-- Replace device/UI boundaries only; execute the real screen methods.
local shown
local stubs = {
  ["ui/uimanager"] = { show = function(__, widget) shown = widget end },
  ["ui/widget/textviewer"] = { new = function(__, widget) return widget end },
  ["widgets/Menu"] = { extend = function(__, widget) return widget end },
  ["device"] = { screen = {} },
  ["gettext+"] = function(text) return "translated:" .. text end,
}
for __, name in ipairs({
  "ui/widget/buttondialog", "ui/widget/confirmbox", "ui/trapper",
  "AvailableSourcesListing", "Backend", "ErrorDialog", "LoadingDialog",
  "SourceSettings", "testing",
}) do
  stubs[name] = {}
end
for name, value in pairs(stubs) do package.loaded[name] = value end
package.path = "frontend/rakuyomi.koplugin/?.lua;" .. package.path
local Listing = require("InstalledSourcesListing")

local failures = 0
local function test(name, run)
  local ok, error_message = pcall(run)
  if ok then
    print("PASS " .. name)
  else
    failures = failures + 1
    print("FAIL " .. name .. ": " .. tostring(error_message))
  end
end

test("populated source list renders translated labels and available versions", function()
  local source = {
    name = "Example", presence = "installed", load = "loaded",
    installed_version = 1, available_version = 2,
    freshness = "update_available", library_manga_count = 3,
  }
  local items = Listing:generateItemTable({ source, source })
  assert(#items == 2)
  assert(items[2].source_status == source)
  assert(items[2].mandatory == "translated:Update available")
  assert(items[2].post_text == "translated:Library manga: 3 · translated:Version: 1 -> 2")
end)

test("empty source list renders without entries", function()
  assert(#Listing:generateItemTable({}) == 0)
end)

test("diagnosis renders translated package and duration labels", function()
  Listing:showDiagnosis({
    source_name = "Example", tested_manga_count = 0,
    probable_identifier_change = false,
    steps = {
      { name = "installed_package", outcome = "passed", message = "Loaded",
        package_label = "Example package", duration_ms = 12 },
      { name = "base_url", outcome = "passed", message = "Reachable", duration_ms = 34 },
    },
  })
  assert(shown.title == "translated:Source diagnosis")
  assert(shown.text:find("translated:Package: Example package", 1, true))
  assert(shown.text:find("translated:Duration: 12 ms", 1, true))
  assert(shown.text:find("translated:Duration: 34 ms", 1, true))
end)

assert(failures == 0, tostring(failures) .. " source screen tests failed")
