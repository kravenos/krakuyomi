local UIManager = require("ui/uimanager")
local ButtonDialog = require("ui/widget/buttondialog")
local ConfirmBox = require("ui/widget/confirmbox")
local TextViewer = require("ui/widget/textviewer")
local Screen = require("device").screen
local Trapper = require("ui/trapper")
local _ = require("gettext+")

local AvailableSourcesListing = require("AvailableSourcesListing")
local Backend = require("Backend")
local Menu = require("widgets/Menu")
local ErrorDialog = require("ErrorDialog")
local LoadingDialog = require("LoadingDialog")
local SourceSettings = require("SourceSettings")
local Testing = require("testing")
local format_languages = require("utils/formatLanguages")

local STATUS_LABELS = {
  installed = _("Installed"),
  missing = _("Missing"),
  loaded = _("Loaded"),
  load_failed = _("Failed to load"),
  not_applicable = _("Not applicable"),
  available = _("Available"),
  unavailable = _("Unavailable"),
  current = _("Current"),
  update_available = _("Update available"),
  healthy = _("Healthy"),
  failing = _("Failing"),
  compatible = _("Compatible"),
  incompatible = _("Incompatible"),
  unknown = _("Unknown"),
  search = _("Search"),
  refresh_chapters = _("Refresh chapters"),
  refresh_details = _("Refresh details"),
  source_list = _("Source list"),
  installed_package = _("Installed package"),
  base_url = _("Base URL"),
  stored_manga = _("Stored manga"),
  stored_manga_lookup = _("Stored manga selection"),
  passed = _("Passed"),
  failed = _("Failed"),
  skipped = _("Skipped"),
  timed_out = _("Timed out"),
  redirected = _("Redirected"),
  changed = _("Identifier changed"),
}

local function label(value)
  return STATUS_LABELS[value] or value or _("Unknown")
end

local function version(value)
  if value == nil then return _("Unknown") end
  return tostring(value)
end

local function cache_age(seconds)
  if seconds == nil then return _("Unknown") end
  if seconds < 60 then return _("Less than a minute") end
  if seconds < 3600 then return math.floor(seconds / 60) .. " " .. _("minutes") end
  if seconds < 86400 then return math.floor(seconds / 3600) .. " " .. _("hours") end
  return math.floor(seconds / 86400) .. " " .. _("days")
end

--- @param source SourceStatus
local function overall_status(source)
  if source.presence == "missing" then return _("Missing") end
  if source.load == "load_failed" then return _("Failed to load") end
  if source.compatibility == "incompatible" then return _("Incompatible") end
  if source.runtime == "failing" then return _("Failing") end
  if source.freshness == "update_available" then return _("Update available") end
  if source.catalog == "unavailable" then return _("Not in source lists") end
  if source.runtime == "healthy" then return _("Healthy") end
  return _("Status unknown")
end

--- @class InstalledSourcesListing: { [any]: any }
--- @field source_statuses SourceStatus[]
--- @field on_return_callback fun(): nil
local InstalledSourcesListing = Menu:extend {
  name = "installed_sources_listing",
  is_enable_shortcut = false,
  is_popout = false,
  title = _("Source status"),
  with_context_menu = true,

  source_statuses = nil,
  on_return_callback = nil,
}

function InstalledSourcesListing:init()
  self.source_statuses = self.source_statuses or {}
  self.title_bar_left_icon = "plus"
  self.onLeftButtonTap = function()
    self:openAvailableSourcesListing()
  end

  self.width = Screen:getWidth()
  self.height = Screen:getHeight()
  local page = self.page
  Menu.init(self)
  self.page = page
  self.paths = { 0 }
end

function InstalledSourcesListing:onClose()
  UIManager:close(self)
  if self.on_return_callback then self.on_return_callback() end
end

--- @private
function InstalledSourcesListing:updateItems()
  if #self.source_statuses > 0 then
    self.item_table = self:generateItemTable(self.source_statuses)
    self.multilines_show_more_text = false
    self.items_per_page = nil
    self.single_line = true
  else
    self.item_table = self:generateEmptyViewItemTable()
    self.multilines_show_more_text = true
    self.items_per_page = 1
    self.single_line = false
  end
  Menu.updateItems(self)
end

--- @private
--- @param source_statuses SourceStatus[]
--- @return table
function InstalledSourcesListing:generateItemTable(source_statuses)
  local item_table = {}
  for _, source in ipairs(source_statuses) do
    local version_text = version(source.installed_version)
    if source.available_version ~= nil and source.freshness == "update_available" then
      version_text = version_text .. " -> " .. version(source.available_version)
    end
    table.insert(item_table, {
      source_status = source,
      text = source.name,
      mandatory = overall_status(source),
      post_text = _("Library manga") .. ": " .. source.library_manga_count .. " · " ..
          _("Version") .. ": " .. version_text,
    })
  end
  return item_table
end

--- @private
function InstalledSourcesListing:generateEmptyViewItemTable()
  return {
    {
      text = _("No installed, failed, or library-referenced sources found.") .. " " ..
          _("Tap the top-left button to find sources."),
      dim = true,
      select_enabled = false,
    }
  }
end

--- @private
--- @param source SourceStatus
function InstalledSourcesListing:showDetails(source)
  local lines = {}
  local function add(name, value)
    if value ~= nil and value ~= "" then table.insert(lines, name .. ": " .. tostring(value)) end
  end

  add(_("Overall status"), overall_status(source))
  add(_("Source ID"), source.source_id)
  add(_("Presence"), label(source.presence))
  add(_("Load"), label(source.load))
  add(_("Source list"), label(source.catalog))
  add(_("Installed version"), version(source.installed_version))
  add(_("Available version"), version(source.available_version))
  add(_("Update status"), label(source.freshness))
  add(_("Runtime health"), label(source.runtime))
  add(_("Compatibility"), label(source.compatibility))
  add(_("Library manga"), source.library_manga_count)
  add(_("Languages"), format_languages(source.languages))
  add(_("Package"), source.package_label)
  add(_("Package format"), source.package_kind)
  add(_("Installed source list ID"), source.installed_list_id)
  add(_("Selected source list ID"), source.selected_list_id)
  add(_("Installed provider"), source.installed_provider_url)
  add(_("Available provider"), source.available_provider_url)
  add(_("Catalog checked"), source.catalog_fetched_at)
  add(_("Catalog age"), cache_age(source.catalog_age_seconds))
  add(_("Catalog warning"), source.catalog_last_fetch_error)
  add(_("Health samples"), source.health_sample_count)
  add(_("Last operation"), label(source.latest_operation))
  add(_("Last operation time"), source.latest_operation_at)
  add(_("Last result category"), source.latest_category)
  add(_("Last result"), source.latest_message)
  add(_("Error"), source.error)

  UIManager:show(TextViewer:new {
    title = source.name,
    text = table.concat(lines, "\n\n"),
  })
end

--- @private
function InstalledSourcesListing:onPrimaryMenuChoice(item)
  self:showDetails(item.source_status)
end

--- @private
--- @param source SourceStatus
function InstalledSourcesListing:openSettings(source)
  local on_return_callback = function()
    self:fetchAndShow(self.on_return_callback)
  end
  SourceSettings:fetchAndShow(source.source_id, on_return_callback)
  UIManager:close(self)
end

--- @private
--- @param source SourceStatus
function InstalledSourcesListing:installSource(source)
  Trapper:wrap(function()
    local response = LoadingDialog:showAndRun(
      source.freshness == "update_available" and _("Updating source...") or _("Installing source..."),
      function()
        return Backend.installSource(
          source.source_id,
          source.selected_list_id,
          source.available_version,
          #source.languages > 0 and source.languages or nil
        )
      end
    )
    if response.type == "ERROR" then
      ErrorDialog:show(response.message)
      return
    end
    if response.body.type == "selection_required" then
      ErrorDialog:show(_("Choose this source in Available sources to select its languages."))
      return
    end
    self:refreshStatuses()
  end)
end

--- @private
--- @param source SourceStatus
function InstalledSourcesListing:removeSource(source)
  local preview = Backend.getSourceUninstallPreview(source.source_id)
  if preview.type == "ERROR" then
    ErrorDialog:show(preview.message)
    return
  end
  local library_manga_count = preview.body.library_manga_count
  UIManager:show(ConfirmBox:new {
    text = _("Do you want to remove the") .. " \"" .. source.name .. "\" " .. _("source?") ..
        "\n\n" .. _("Library manga affected") .. ": " .. library_manga_count .. ". " ..
        _("Library entries and downloaded chapters will be kept."),
    ok_text = _("Remove"),
    ok_callback = function()
      local response = Backend.uninstallSource(source.source_id, library_manga_count)
      if response.type == "ERROR" then
        ErrorDialog:show(response.message)
        return
      end
      self:refreshStatuses()
    end
  })
end

--- @private
--- @param diagnosis SourceDiagnosis
function InstalledSourcesListing:showDiagnosis(diagnosis)
  local lines = {
    _("Source") .. ": " .. diagnosis.source_name,
    _("Stored manga tested") .. ": " .. diagnosis.tested_manga_count,
    _("Possible identifier change") .. ": " ..
        (diagnosis.probable_identifier_change and _("Yes") or _("No")),
  }
  for _, step in ipairs(diagnosis.steps) do
    local heading = label(step.name)
    if step.tested_item ~= nil then heading = heading .. " " .. step.tested_item end
    local details = { heading .. ": " .. label(step.outcome), step.message }
    if step.http_status ~= nil then
      table.insert(details, "HTTP: " .. step.http_status)
    end
    if step.package_label ~= nil then
      table.insert(details, _("Package") .. ": " .. step.package_label)
    end
    table.insert(details, _("Duration") .. ": " .. step.duration_ms .. " ms")
    table.insert(lines, table.concat(details, "\n"))
  end
  table.insert(lines, _("Diagnosis did not change library entries, source packages, or manga identifiers."))

  UIManager:show(TextViewer:new {
    title = _("Source diagnosis"),
    text = table.concat(lines, "\n\n"),
  })
end

--- @private
--- @param source SourceStatus
function InstalledSourcesListing:diagnoseSource(source)
  Trapper:wrap(function()
    local response = LoadingDialog:showAndRun(
      _("Checking source..."),
      function() return Backend.diagnoseSource(source.source_id, true) end
    )
    if response.type == "ERROR" then
      ErrorDialog:show(response.message)
      return
    end
    self:showDiagnosis(response.body)
    self:refreshStatuses()
  end)
end

--- @private
function InstalledSourcesListing:onContextMenuChoice(item)
  local source = item.source_status
  local dialog
  local buttons = {}
  local function action(text, callback)
    table.insert(buttons, {
      {
        text = text,
        callback = function()
          UIManager:close(dialog)
          callback()
        end,
      },
    })
  end

  action(_("Details"), function() self:showDetails(source) end)
  if source.load == "loaded" then
    action(_("Diagnose"), function() self:diagnoseSource(source) end)
    action(_("Settings"), function() self:openSettings(source) end)
  end
  if source.selected_list_id ~= nil and source.available_version ~= nil and
      (source.presence == "missing" or source.load == "load_failed" or source.freshness == "update_available") then
    action(source.freshness == "update_available" and _("Update") or _("Install"),
      function() self:installSource(source) end)
  end
  if source.presence == "installed" then
    action(_("Remove"), function() self:removeSource(source) end)
  end
  action(_("Cancel"), function() end)

  dialog = ButtonDialog:new {
    title = source.name,
    buttons = buttons,
  }
  UIManager:show(dialog)
end

--- @private
function InstalledSourcesListing:refreshStatuses()
  local response = Backend.listSourceStatuses()
  if response.type == "ERROR" then
    ErrorDialog:show(response.message)
    return
  end
  self.source_statuses = response.body
  self:updateItems()
end

--- @private
function InstalledSourcesListing:onReturn()
  table.remove(self.paths)
  self:onClose()
end

--- @private
function InstalledSourcesListing:openAvailableSourcesListing()
  Trapper:wrap(function()
    local onReturnCallback = function()
      self:fetchAndShow(self.on_return_callback)
    end
    AvailableSourcesListing:fetchAndShow(onReturnCallback)
    UIManager:close(self)
  end)
end

--- Fetches and shows the complete source status screen.
--- @param onReturnCallback fun(): nil
function InstalledSourcesListing:fetchAndShow(onReturnCallback)
  local response = Backend.listSourceStatuses()
  if response.type == "ERROR" then
    ErrorDialog:show(response.message)
    return
  end

  local ui = InstalledSourcesListing:new {
    source_statuses = response.body,
    on_return_callback = onReturnCallback,
    covers_fullscreen = true,
    page = self.page,
  }
  ui.on_return_callback = onReturnCallback
  UIManager:show(ui)
  Testing:emitEvent("installed_sources_listing_shown")
end

return InstalledSourcesListing
