-- Run from the repository root: lua5.1 frontend/tests/cover_rendering_test.lua
-- Exercise the real cover builder with in-memory image and widget boundaries.
-- ImageWidget semantics follow KOReader v2026.07.2: _render returns early when
-- _bb exists, getSize honors viewport dimensions, and free releases owned buffers.
local buffers, decoded_paths
local source_width, source_height = 600, 900
local function buffer(width, height)
  local bb = { w = width, h = height, freed = false }
  function bb:getWidth() assert(not self.freed, "read freed image") return self.w end
  function bb:getHeight() assert(not self.freed, "read freed image") return self.h end
  function bb:free() assert(not self.freed, "image freed twice") self.freed = true end
  buffers[#buffers + 1] = bb
  return bb
end

local ImageWidget = { image_disposable = true, _bb_disposable = true }
function ImageWidget:new(value) return setmetatable(value, { __index = self }) end
function ImageWidget:_loadfile()
  decoded_paths[#decoded_paths + 1] = self.file
  self._bb = buffer(source_width, source_height)
  self._bb_disposable = self.file_do_cache == false
end
function ImageWidget:_render()
  if self._bb then return end
  if self.image then
    self._bb = self.image
    self._bb_disposable = self.image_disposable
  else
    self:_loadfile()
  end
  self._initial_scale_factor = self.scale_factor
  local w, h = self._bb:getWidth(), self._bb:getHeight()
  self._img_w, self._img_h = w, h
  if self.scale_factor == 0 then
    self.scale_factor = self.width and self.height and math.min(self.width / w, self.height / h) or 1
  end
  local target_w, target_h = w, h
  if self.scale_factor and self.scale_factor ~= 1 then
    target_w, target_h = math.floor(w * self.scale_factor), math.floor(h * self.scale_factor)
  elseif self.scale_factor == nil and self.width and self.height then
    target_w, target_h = self.width, self.height
  end
  if target_w ~= w or target_h ~= h then
    if self._bb_disposable then self._bb:free() end
    self._bb = buffer(target_w, target_h)
    self._bb_disposable = true
  end
  self._bb_w, self._bb_h = self._bb:getWidth(), self._bb:getHeight()
end
function ImageWidget:getSize()
  self:_render()
  return { w = self.width or self._bb:getWidth(), h = self.height or self._bb:getHeight() }
end
function ImageWidget:getCurrentWidth() return self._bb:getWidth() end
function ImageWidget:getCurrentHeight() return self._bb:getHeight() end
function ImageWidget:free()
  if self._bb and self._bb_disposable then self._bb:free() end
  self._bb = nil
  self.scale_factor = self._initial_scale_factor
end

local Container = {}
function Container:new(value) return setmetatable(value, { __index = self }) end
function Container:extend(value) return self:new(value) end
function Container:free()
  for __, child in ipairs(self) do
    if child.free then child:free() end
  end
end
local stubs = {
  ["MenuItem"] = Container,
  ["ui/widget/imagewidget"] = ImageWidget,
  ["ui/geometry"] = Container,
  ["ui/size"] = { border = { thin = 1 } },
  ["device"] = { screen = { scaleBySize = function(__, size) return size end } },
  ["ffi/blitbuffer"] = { COLOR_GRAY_9 = 9 },
  ["ui/font"] = { getFace = function() return {} end },
  ["logger"] = {},
  ["optmath"] = {},
}
for __, name in ipairs({
  "ui/gesturerange", "ui/widget/container/rightcontainer", "ui/widget/horizontalgroup",
  "ui/widget/horizontalspan", "ui/widget/verticalgroup", "ui/widget/verticalspan",
  "ui/widget/textboxwidget", "ui/widget/textwidget", "ui/widget/container/framecontainer",
  "ui/widget/container/leftcontainer", "ui/widget/container/underlinecontainer",
  "ui/widget/overlapgroup", "ui/widget/container/centercontainer",
}) do stubs[name] = Container end
for name, value in pairs(stubs) do package.loaded[name] = value end
package.path = "frontend/rakuyomi.koplugin/?.lua;" .. package.path
local MenuItemCover = require("patch/MenuItemCover")

local function new_cover(path)
  local item = setmetatable({
    entry = { manga_cover = path or "file:///covers/manga.jpg" },
    menu = {}, dimen = { h = 180 },
  }, { __index = MenuItemCover })
  return item:genCover(120, 180), item
end
local function live_buffers()
  local count = 0
  for __, bb in ipairs(buffers) do if not bb.freed then count = count + 1 end end
  return count
end
local failures = 0
local function test(name, run)
  buffers, decoded_paths = {}, {}
  source_width, source_height = 600, 900
  local ok, message = pcall(run)
  if ok then print("PASS " .. name) else
    failures = failures + 1
    print("FAIL " .. name .. ": " .. tostring(message))
  end
end

test("one file decode per displayed cover", function()
  local cover = new_cover()
  assert(#decoded_paths == 1, "cover decoded " .. #decoded_paths .. " times")
  cover:free()
end)

test("freeing the cover tree releases all decoded image buffers", function()
  local cover = new_cover()
  cover:free()
  assert(live_buffers() == 0, "discarded size-probe image was not freed with the cover")
end)

for __, dimensions in ipairs({
  { name = "portrait", w = 600, h = 900, fit_w = 120, fit_h = 180 },
  { name = "landscape", w = 1200, h = 600, fit_w = 120, fit_h = 60 },
  { name = "narrow", w = 300, h = 900, fit_w = 60, fit_h = 180 },
}) do
  test(dimensions.name .. " cover keeps aspect ratio and border hugs actual image", function()
    source_width, source_height = dimensions.w, dimensions.h
    local cover, item = new_cover("file:///covers/My%20Manga%25.jpg")
    local frame, image = cover[1], cover[1][1]
    assert(decoded_paths[1] == "/covers/My Manga%.jpg", "file URL was not decoded")
    assert(image._bb:getWidth() == dimensions.fit_w and image._bb:getHeight() == dimensions.fit_h,
      "image stretched or no longer fits available cover area")
    local image_size = image:getSize()
    assert(image_size.w == dimensions.fit_w and image_size.h == dimensions.fit_h,
      "image viewport exceeds fitted image")
    assert(frame.width == dimensions.fit_w + 2 and frame.height == dimensions.fit_h + 2,
      "border surrounds viewport rather than actual image")
    assert(item._has_cover_image and item.menu._has_cover_images)
    cover:free()
  end)
end

test("repeated cover creation and disposal leaves no retained or freed images", function()
  for iteration = 1, 5 do
    source_width, source_height = 300 * iteration, 900
    local cover = new_cover()
    local image = cover[1][1]
    image:getSize()
    assert(not image._bb.freed, "displayed image was already freed")
    cover:free()
    assert(live_buffers() == 0, "buffers accumulate across cover refreshes")
  end
end)

test("same image widget can be freed and rendered again at the same fitted size", function()
  source_width, source_height = 1200, 600
  local cover = new_cover()
  local image = cover[1][1]
  local original_size = image:getSize()
  assert(#decoded_paths == 1, "initial render did not decode exactly once")
  for render_count = 2, 4 do
    local previous_buffer = image._bb
    image:free()
    assert(live_buffers() == 0, "free retained an owned buffer")
    local size = image:getSize()
    assert(size.w == original_size.w and size.h == original_size.h,
      "same-widget rerender changed the fitted size")
    assert(image._bb ~= previous_buffer and not image._bb.freed, "rerender reused a freed buffer")
    assert(#decoded_paths == render_count, "rerender did not decode exactly once")
  end
  cover:free()
  assert(live_buffers() == 0, "final widget disposal retained a buffer")
end)

test("missing cover keeps placeholder and performs no image decode", function()
  local cover, item = new_cover("https://example.invalid/not-a-local-cover.jpg")
  assert(#decoded_paths == 0 and not item._has_cover_image)
  assert(cover[1][1][1].text ~= nil, "placeholder disappeared")
  cover:free()
end)

assert(failures == 0, tostring(failures) .. " cover rendering tests failed")
