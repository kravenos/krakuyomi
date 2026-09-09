-- Run from the repository root: lua5.1 frontend/tests/reading_direction_test.lua
-- Execute the real reader and preference helpers; fake KOReader's document lifecycle.
local pending = {}
local ReaderUI = {}
local UIManager = {
  nextTick = function(__, callback) pending[#pending + 1] = callback end,
  broadcastEvent = function() end,
}
local settings = {}
G_reader_settings = {
  readSetting = function(__, key) return settings[key] end,
  isTrue = function(__, key) return settings[key] == true end,
  nilOrTrue = function(__, key) return settings[key] ~= false end,
  nilOrFalse = function(__, key) return not settings[key] end,
}
MangaViewer = { DefaultViewer = 0, Rtl = 1, Ltr = 2, Vertical = 3, Scroll = 4 }

local stubs = {
  ["apps/reader/readerui"] = ReaderUI,
  ["ui/uimanager"] = UIManager,
  ["ui/widget/container/widgetcontainer"] = { new = function(__, value) return value end },
  ["ui/event"] = { new = function(__, name, ...) return { name = name, args = { ... } } end },
  ["logger"] = { warn = function() end },
  ["gettext+"] = function(value) return value end,
  ["extensions/CbzDocument"] = { registerChapterFile = function() end },
  ["testing"] = { emitEvent = function() end },
  ["ffi/util"] = {},
}
for __, name in ipairs({ "ui/widget/confirmbox", "ui/widget/configdialog", "ui/trapper", "Backend" }) do
  stubs[name] = {}
end
for name, value in pairs(stubs) do package.loaded[name] = value end
package.path = "frontend/rakuyomi.koplugin/?.lua;" .. package.path
local MangaReader = require("MangaReader")

local function drain_ticks()
  local count = 0
  while #pending > 0 do
    count = count + 1
    assert(count < 30, "reader never settled")
    local callback = table.remove(pending, 1)
    callback()
  end
end

local function open_document(path, after_open_callback)
  local ui = {
    {}, -- ReaderUI already has child widgets before RakuYomi inserts its listener.
    name = "ReaderUI",
    document = { file = path, configurable = { page_scroll = 0 } },
    view = { inverse_reading_order = false, direction_changes = 0 },
    config = { options = { prefix = "kopt" } },
    menu = {
      menu_items = { filemanager = { callback = function() end } },
      registerToMainMenu = function() end,
    },
    post_init_callbacks = {},
  }
  function ui.view:onToggleReadingOrder(rtl)
    self.inverse_reading_order = rtl
    self.direction_changes = self.direction_changes + 1
  end
  function ui:registerPostInitCallback(callback)
    self.post_init_callbacks[#self.post_init_callbacks + 1] = callback
  end
  function ui:handleEvent(event)
    if event.name == "ConfigChange" then
      self.document.configurable[event.args[1]] = event.args[2]
    elseif event.name == "SetScrollMode" then
      self.view.page_scroll = event.args[1]
    end
  end
  function ui:switchDocument(next_path, __, callback)
    -- KOReader closes now, then opens the new ReaderUI on a later tick.
    MangaReader:onReaderUiCloseWidget()
    self.document = nil
    ReaderUI.instance = nil
    UIManager:nextTick(function()
      ReaderUI:showReader(next_path, nil, nil, nil, callback)
    end)
  end
  MangaReader:initializeFromReaderUI(ui)
  for __, callback in ipairs(ui.post_init_callbacks) do callback() end
  -- ReaderUI:init invokes self:after_open_callback() before showReader assigns
  -- ReaderUI.instance. Passing this ui matches KOReader v2026.07.2.
  if after_open_callback then after_open_callback(ui) end
  ReaderUI.instance = ui
  return ui
end

function ReaderUI:showReader(path, __, __, __, after_open_callback)
  if self.defer_first_open then
    self.defer_first_open = false
    -- A queued reader startup should not let preferences run before it exists.
    UIManager:nextTick(function()
      UIManager:nextTick(function() open_document(path, after_open_callback) end)
    end)
  else
    open_document(path, after_open_callback)
  end
end

local function reset(direction, style)
  pending = {}
  settings = {
    rakuyomi_reading_direction = direction,
    rakuyomi_page_turn_style = style,
  }
  ReaderUI.instance = nil
  ReaderUI.defer_first_open = false
  MangaReader:clean()
  MangaReader.is_showing = false
  MangaReader.is_switching_document = false
end

local function show_chapter(path, viewer)
  MangaReader:show {
    path = path,
    chapter = { source_id = "test", manga_id = "manga", chapter_id = path },
    chapters = {},
    viewer = viewer,
    state_viewer = false,
    on_rtl_changed = function() end,
  }
end

local failures = 0
local function test(name, run)
  local ok, message = pcall(run)
  if ok then
    print("PASS " .. name)
  else
    failures = failures + 1
    print("FAIL " .. name .. ": " .. tostring(message))
  end
end

for __, case in ipairs({
  { direction = "ltr", viewer = "Rtl", inverse = false },
  { direction = "rtl", viewer = "Ltr", inverse = true },
}) do
  test("explicit " .. case.direction .. " wins over source mode on first open", function()
    reset(case.direction)
    show_chapter("first.cbz", case.viewer)
    drain_ticks()
    assert(ReaderUI.instance.view.inverse_reading_order == case.inverse,
      "first chapter did not use the explicit direction")
  end)

  test("explicit " .. case.direction .. " survives delayed next-chapter initialization", function()
    reset(case.direction)
    show_chapter("first.cbz", case.viewer)
    drain_ticks()
    local previous = ReaderUI.instance
    local previous_changes = previous.view.direction_changes
    show_chapter("second.cbz", case.viewer)
    drain_ticks()
    local current = ReaderUI.instance
    assert(current ~= previous and current.document.file == "second.cbz", "new chapter did not open")
    assert(current.view.inverse_reading_order == case.inverse,
      "next chapter reverted to the source direction")
    assert(previous.view.direction_changes == previous_changes, "closed reader was changed")
  end)

  test("explicit " .. case.direction .. " waits for delayed first-reader initialization", function()
    reset(case.direction)
    ReaderUI.defer_first_open = true
    show_chapter("first.cbz", case.viewer)
    drain_ticks()
    assert(ReaderUI.instance.document.file == "first.cbz")
    assert(ReaderUI.instance.view.inverse_reading_order == case.inverse,
      "delayed first chapter did not use the explicit direction")
  end)
end

test("unset direction preserves the source viewer on both chapters", function()
  reset(nil)
  show_chapter("first.cbz", "Rtl")
  drain_ticks()
  assert(ReaderUI.instance.view.inverse_reading_order == true)
  show_chapter("second.cbz", "Ltr")
  drain_ticks()
  assert(ReaderUI.instance.view.inverse_reading_order == false)
end)

test("explicit page-turn style survives delayed chapter initialization", function()
  reset("ltr", "scroll")
  show_chapter("first.cbz", "Rtl")
  drain_ticks()
  assert(ReaderUI.instance.document.configurable.page_scroll == 1)
  show_chapter("second.cbz", "Rtl")
  drain_ticks()
  assert(ReaderUI.instance.document.configurable.page_scroll == 1,
    "next chapter lost the explicit scroll style")
  assert(ReaderUI.instance.view.inverse_reading_order == false)
end)

test("reader closing before pending preferences does not change the closed view", function()
  reset("ltr")
  show_chapter("first.cbz", "Rtl")
  local closed = ReaderUI.instance
  local previous_changes = closed.view.direction_changes
  MangaReader:onReaderUiCloseWidget()
  ReaderUI.instance = nil
  drain_ticks()
  assert(closed.view.direction_changes == previous_changes)
end)

assert(failures == 0, tostring(failures) .. " reading direction tests failed")
