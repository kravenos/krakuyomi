local ButtonDialog = require("ui/widget/buttondialog")
local ConfirmBox = require("ui/widget/confirmbox")
local InfoMessage = require("ui/widget/infomessage")
local InputDialog = require("ui/widget/inputdialog")
local TextViewer = require("ui/widget/textviewer")
local Trapper = require("ui/trapper")
local UIManager = require("ui/uimanager")
local Screen = require("device").screen
local rapidjson = require("rapidjson")
local util = require("util")
local _ = require("gettext+")

local Backend = require("Backend")
local ErrorDialog = require("ErrorDialog")
local LoadingDialog = require("LoadingDialog")
local Menu = require("widgets/Menu")
local Paths = require("Paths")
local Testing = require("testing")

local EXPORT_SCHEMA_VERSION = 1
local EXPORT_FILENAME = "source-lists.json"

--- @class SourceListsListing: { [any]: any }
--- @field settings Settings
--- @field catalog_statuses table<string, SourceCatalogSummary>
--- @field on_return_callback fun(): nil
local SourceListsListing = Menu:extend {
  name = "source_lists_listing",
  is_enable_shortcut = false,
  is_popout = false,
  title = _("Source lists"),

  settings = nil,
  catalog_statuses = nil,
  on_return_callback = nil,
}

--- @type table<string, string>
local SOURCE_LIST_TYPE_LABELS = {
  aidoku = "Aidoku",
  lnreader = "LNReader",
  mangayomi = "MangaYomi",
  keiyoushi = "Keiyoushi",
}

local function isEnabled(source_list)
  return source_list.enabled ~= false
end

local function cloneLists(source_lists)
  return util.tableDeepCopy(source_lists or {})
end

local function sourceListKey(source_list)
  return (source_list.type or "aidoku") .. "\n" .. source_list.url
end

function SourceListsListing:init()
  self.catalog_statuses = self.catalog_statuses or {}
  self.title_bar_left_icon = "plus"
  self.onLeftButtonTap = function()
    self:showListTools()
  end

  self.width = Screen:getWidth()
  self.height = Screen:getHeight()
  Menu.init(self)
  self:updateItems()

  -- see `ChapterListing` for an explanation on this
  self.paths = { 0 }
end

function SourceListsListing:onClose()
  UIManager:close(self)
  if self.on_return_callback then
    self.on_return_callback()
  end
end

--- @private
function SourceListsListing:updateItems()
  local source_lists = self.settings.source_lists or {}
  if #source_lists > 0 then
    self.item_table = self:generateItemTable(source_lists)
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
--- @param source_list SourceList
--- @param status SourceCatalogSummary|nil
--- @return string
function SourceListsListing:formatStatus(source_list, status)
  if not isEnabled(source_list) then
    return _("Disabled")
  end
  if status == nil or not status.cached then
    return _("Not refreshed")
  end
  if status.last_fetch_error ~= nil then
    return _("Cached") .. " · " .. status.candidate_count
  end
  return status.candidate_count .. " " .. _("sources")
end

--- @private
--- @param source_lists SourceList[]
--- @return table
function SourceListsListing:generateItemTable(source_lists)
  local item_table = {}
  for index, source_list in ipairs(source_lists) do
    local status = nil
    for _, catalog_status in pairs(self.catalog_statuses) do
      if catalog_status.configured_order == index - 1 then
        status = catalog_status
        break
      end
    end
    table.insert(item_table, {
      source_list = source_list,
      status = status,
      index = index,
      text = source_list.url,
      mandatory = self:formatStatus(source_list, status),
      post_text = SOURCE_LIST_TYPE_LABELS[source_list.type] or SOURCE_LIST_TYPE_LABELS.aidoku,
    })
  end

  return item_table
end

--- @private
function SourceListsListing:generateEmptyViewItemTable()
  return {
    {
      text = _("No source lists configured.") .. " " .. _("Use the top-left button to add or import one."),
      dim = true,
      select_enabled = false,
    }
  }
end

--- @private
function SourceListsListing:showListTools()
  local dialog
  dialog = ButtonDialog:new {
    title = _("Source list tools"),
    buttons = {
      {
        {
          text = _("Add"),
          callback = function()
            UIManager:close(dialog)
            self:showAddSourceList()
          end,
        },
        {
          text = _("Import"),
          callback = function()
            UIManager:close(dialog)
            self:importSourceLists()
          end,
        },
      },
      {
        {
          text = _("Export"),
          callback = function()
            UIManager:close(dialog)
            self:exportSourceLists()
          end,
        },
        {
          text = _("Cancel"),
          callback = function()
            UIManager:close(dialog)
          end,
        },
      },
    },
  }
  UIManager:show(dialog)
end

--- @private
--- @param item table
function SourceListsListing:onPrimaryMenuChoice(item)
  local dialog
  local enabled = isEnabled(item.source_list)
  dialog = ButtonDialog:new {
    title = item.source_list.url,
    buttons = {
      {
        {
          text = _("Validate"),
          callback = function()
            UIManager:close(dialog)
            self:validateSourceList(item.source_list)
          end,
        },
        {
          text = _("Refresh"),
          callback = function()
            UIManager:close(dialog)
            self:refreshSourceList(item)
          end,
        },
      },
      {
        {
          text = _("Move up"),
          enabled = item.index > 1,
          callback = function()
            UIManager:close(dialog)
            self:moveSourceList(item.index, -1)
          end,
        },
        {
          text = _("Move down"),
          enabled = item.index < #(self.settings.source_lists or {}),
          callback = function()
            UIManager:close(dialog)
            self:moveSourceList(item.index, 1)
          end,
        },
      },
      {
        {
          text = enabled and _("Disable") or _("Enable"),
          callback = function()
            UIManager:close(dialog)
            if enabled then
              self:previewListChange(item, "disable")
            else
              self:setSourceListEnabled(item.index, true)
            end
          end,
        },
        {
          text = _("Remove"),
          callback = function()
            UIManager:close(dialog)
            self:previewListChange(item, "remove")
          end,
        },
      },
      {
        {
          text = _("Cancel"),
          callback = function()
            UIManager:close(dialog)
          end,
        },
      },
    },
  }

  UIManager:show(dialog)
end

--- @private
function SourceListsListing:showAddSourceList()
  local input_dialog
  input_dialog = InputDialog:new {
    title = _("Add source list"),
    input_hint = _("URL of the source list (index.json or plugins index)"),
    description = _("The list will be fetched and validated before it is saved."),
    buttons = {
      {
        {
          text = _("Cancel"),
          id = "close",
          callback = function()
            UIManager:close(input_dialog)
          end,
        },
        {
          text = _("Next"),
          is_enter_default = true,
          callback = function()
            local url = input_dialog:getInputText()
            UIManager:close(input_dialog)
            if url == "" then
              ErrorDialog:show(_("Please enter a source list URL."))
              return
            end
            if not url:lower():match("^https?://") then
              ErrorDialog:show(_("Source list URLs must start with http:// or https://."))
              return
            end
            self:chooseSourceListType(url)
          end,
        },
      }
    }
  }

  UIManager:show(input_dialog)
  input_dialog:onShowKeyboard()
end

--- @private
--- @param url string
function SourceListsListing:chooseSourceListType(url)
  local dialog
  local function choose(source_type)
    UIManager:close(dialog)
    self:addSourceList(url, source_type)
  end
  dialog = ButtonDialog:new {
    title = url,
    buttons = {
      {
        { text = "Aidoku", callback = function() choose("aidoku") end },
        { text = "LNReader", callback = function() choose("lnreader") end },
      },
      {
        { text = "MangaYomi", callback = function() choose("mangayomi") end },
        { text = "Keiyoushi", callback = function() choose("keiyoushi") end },
      },
      {
        { text = _("Cancel"), callback = function() UIManager:close(dialog) end },
      },
    },
  }
  UIManager:show(dialog)
end

--- @private
--- @param url string
--- @param source_type "aidoku"|"lnreader"|"mangayomi"|"keiyoushi"
function SourceListsListing:addSourceList(url, source_type)
  local source_list = {
    url = url,
    type = source_type,
    enabled = true,
  }
  Trapper:wrap(function()
    local response = LoadingDialog:showAndRun(
      _("Validating source list..."),
      function() return Backend.validateSourceCatalog(source_list) end
    )
    if response.type == "ERROR" then
      ErrorDialog:show(response.message)
      return
    end
    for _, status in pairs(self.catalog_statuses) do
      if status.list_id == response.body.list_id then
        ErrorDialog:show(_("This source list is already configured."))
        return
      end
    end

    local source_lists = cloneLists(self.settings.source_lists)
    table.insert(source_lists, source_list)
    if self:saveSourceLists(source_lists) then
      UIManager:show(InfoMessage:new {
        text = _("Source list validated") .. ": " .. response.body.candidate_count .. " " .. _("sources"),
      })
    end
  end)
end

--- @private
--- @param source_list SourceList
function SourceListsListing:validateSourceList(source_list)
  Trapper:wrap(function()
    local response = LoadingDialog:showAndRun(
      _("Validating source list..."),
      function() return Backend.validateSourceCatalog(source_list) end
    )
    if response.type == "ERROR" then
      ErrorDialog:show(response.message)
      return
    end
    UIManager:show(InfoMessage:new {
      text = _("Source list is valid") .. ": " .. response.body.candidate_count .. " " .. _("sources"),
    })
  end)
end

--- @private
--- @param item table
function SourceListsListing:refreshSourceList(item)
  if item.status == nil then
    ErrorDialog:show(_("Source list status is not available. Reopen this screen and try again."))
    return
  end
  Trapper:wrap(function()
    local response = LoadingDialog:showAndRun(
      _("Refreshing source list..."),
      function() return Backend.refreshSourceCatalog(item.status.list_id) end
    )
    if response.type == "ERROR" then
      ErrorDialog:show(response.message)
      return
    end
    self.catalog_statuses[response.body.list_id] = response.body
    self:updateItems()
    if response.body.last_fetch_error ~= nil then
      UIManager:show(InfoMessage:new {
        text = _("Refresh failed. The last valid cached list is still in use."),
      })
    else
      UIManager:show(InfoMessage:new {
        text = _("Source list refreshed") .. ": " .. response.body.candidate_count .. " " .. _("sources"),
      })
    end
  end)
end

--- @private
--- @param index number
--- @param direction number
function SourceListsListing:moveSourceList(index, direction)
  local target = index + direction
  local source_lists = cloneLists(self.settings.source_lists)
  if target < 1 or target > #source_lists then
    return
  end
  source_lists[index], source_lists[target] = source_lists[target], source_lists[index]
  self:saveSourceLists(source_lists)
end

--- @private
--- @param index number
--- @param enabled boolean
function SourceListsListing:setSourceListEnabled(index, enabled)
  local source_lists = cloneLists(self.settings.source_lists)
  source_lists[index].enabled = enabled
  self:saveSourceLists(source_lists)
end

--- @private
--- @param index number
function SourceListsListing:removeSourceList(index)
  local source_lists = cloneLists(self.settings.source_lists)
  table.remove(source_lists, index)
  self:saveSourceLists(source_lists)
end

--- @private
--- @param affected_sources CatalogCoverageSource[]
--- @param coverage_known boolean
--- @return string
function SourceListsListing:formatCoverageImpact(affected_sources, coverage_known)
  if not coverage_known then
    return _("Coverage could not be determined because this list has no valid cache. Refresh it before continuing if you need a complete preview.")
  end
  if #affected_sources == 0 then
    return _("No installed or missing source will lose catalog coverage.")
  end
  local lines = {
    _("Sources that will lose catalog coverage") .. ": " .. #affected_sources,
  }
  for index, source in ipairs(affected_sources) do
    if index > 8 then
      table.insert(lines, _("More") .. ": " .. (#affected_sources - 8))
      break
    end
    local state = source.presence == "installed" and _("installed") or _("missing")
    table.insert(lines, "• " .. source.name .. " (" .. state .. ", " ..
      source.library_manga_count .. " " .. _("library manga") .. ")")
  end
  return table.concat(lines, "\n")
end

--- @private
--- @param item table
--- @param action "disable"|"remove"
function SourceListsListing:previewListChange(item, action)
  local function showConfirmation(affected_sources, coverage_known)
    local verb = action == "disable" and _("Disable") or _("Remove")
    UIManager:show(ConfirmBox:new {
      text = verb .. " " .. item.source_list.url .. "?\n\n" ..
          self:formatCoverageImpact(affected_sources, coverage_known) .. "\n\n" ..
          _("This will not uninstall any source."),
      ok_text = verb,
      ok_callback = function()
        if action == "disable" then
          self:setSourceListEnabled(item.index, false)
        else
          self:removeSourceList(item.index)
        end
      end,
    })
  end

  if not isEnabled(item.source_list) then
    showConfirmation({}, true)
    return
  end
  if item.status == nil then
    ErrorDialog:show(_("Source list status is not available. Reopen this screen and try again."))
    return
  end
  local response = Backend.getSourceCatalogChangePreview(item.status.list_id)
  if response.type == "ERROR" then
    ErrorDialog:show(response.message)
    return
  end
  showConfirmation(response.body.affected_sources, response.body.coverage_known)
end

--- @private
--- @param source_lists SourceList[]
--- @return boolean
function SourceListsListing:saveSourceLists(source_lists)
  local previous = self.settings.source_lists
  self.settings.source_lists = source_lists
  local response = Backend.setSettings(self.settings)
  if response.type == "ERROR" then
    self.settings.source_lists = previous
    ErrorDialog:show(response.message)
    return false
  end
  self:refreshCatalogStatuses()
  return true
end

--- @private
function SourceListsListing:refreshCatalogStatuses()
  local response = Backend.listSourceCatalogs()
  if response.type == "ERROR" then
    ErrorDialog:show(response.message)
    return
  end
  self.catalog_statuses = {}
  for _, status in ipairs(response.body) do
    self.catalog_statuses[status.list_id] = status
  end
  self:updateItems()
end

--- @private
--- @return string
function SourceListsListing:getExportPath()
  return Paths.getHomeDirectory() .. "/" .. EXPORT_FILENAME
end

--- @private
function SourceListsListing:exportSourceLists()
  local document = {
    schema_version = EXPORT_SCHEMA_VERSION,
    source_lists = self.settings.source_lists or {},
  }
  local serialized = rapidjson.encode(document)
  local export_path = self:getExportPath()
  local temporary_path = export_path .. ".tmp"
  local file, open_error = io.open(temporary_path, "wb")
  if file == nil then
    ErrorDialog:show(_("Could not create the export file.") .. " " .. tostring(open_error))
    return
  end
  local wrote, write_error = file:write(serialized .. "\n")
  local closed, close_error = file:close()
  if wrote == nil or not closed then
    os.remove(temporary_path)
    ErrorDialog:show(_("Could not write the export file.") .. " " .. tostring(write_error or close_error))
    return
  end
  local renamed, rename_error = os.rename(temporary_path, export_path)
  if not renamed then
    os.remove(temporary_path)
    ErrorDialog:show(_("Could not finish the export file.") .. " " .. tostring(rename_error))
    return
  end

  UIManager:show(TextViewer:new {
    title = _("Source lists exported"),
    text = _("Saved to") .. ":\n" .. export_path .. "\n\n" ..
        _("The file contains exact URLs. Keep it private if a URL contains credentials.") ..
        "\n\n" .. serialized,
  })
end

--- @private
--- @param document any
--- @return SourceList[]|nil, string|nil
function SourceListsListing:validateImportDocument(document)
  if type(document) ~= "table" or document.schema_version ~= EXPORT_SCHEMA_VERSION or
      type(document.source_lists) ~= "table" then
    return nil, _("The import file is not a supported source-list export.")
  end
  local source_lists = {}
  local seen = {}
  for index, source_list in ipairs(document.source_lists) do
    if type(source_list) ~= "table" or type(source_list.url) ~= "string" or
        source_list.url == "" or not source_list.url:lower():match("^https?://") or
        SOURCE_LIST_TYPE_LABELS[source_list.type or "aidoku"] == nil or
        (source_list.enabled ~= nil and type(source_list.enabled) ~= "boolean") then
      return nil, _("Invalid source list at position") .. " " .. index .. "."
    end
    local source_type = source_list.type or "aidoku"
    local key = sourceListKey(source_list)
    if seen[key] then
      return nil, _("The import file contains a duplicate source list.")
    end
    seen[key] = true
    table.insert(source_lists, {
      url = source_list.url,
      type = source_type,
      enabled = source_list.enabled ~= false,
    })
  end
  return source_lists
end

--- @private
function SourceListsListing:importSourceLists()
  local export_path = self:getExportPath()
  local file, open_error = io.open(export_path, "rb")
  if file == nil then
    ErrorDialog:show(_("Could not open the import file.") .. "\n" .. export_path .. "\n" .. tostring(open_error))
    return
  end
  local contents = file:read("*a")
  file:close()
  local ok, document = pcall(rapidjson.decode, contents)
  if not ok then
    ErrorDialog:show(_("The import file is not valid JSON."))
    return
  end
  local source_lists, validation_error = self:validateImportDocument(document)
  if source_lists == nil then
    ErrorDialog:show(validation_error)
    return
  end
  local imported_by_key = {}
  for _, source_list in ipairs(source_lists) do
    imported_by_key[sourceListKey(source_list)] = source_list
  end
  for _, current in ipairs(self.settings.source_lists or {}) do
    local imported = imported_by_key[sourceListKey(current)]
    if isEnabled(current) and (imported == nil or not isEnabled(imported)) then
      ErrorDialog:show(
        _("The import would remove or disable an active source list.") .. "\n\n" ..
        _("Disable or remove that list from this screen first so you can review its coverage warning.")
      )
      return
    end
  end

  UIManager:show(ConfirmBox:new {
    text = _("Replace the current source lists with the imported file?") .. "\n\n" ..
        _("Lists to import") .. ": " .. #source_lists .. ". " ..
        _("Order, type, enabled state, and exact URLs will be preserved."),
    ok_text = _("Import"),
    ok_callback = function()
      if self:saveSourceLists(source_lists) then
        UIManager:show(InfoMessage:new { text = _("Source lists imported.") })
      end
    end,
  })
end

--- @private
function SourceListsListing:onReturn()
  table.remove(self.paths)
  self:onClose()
end

--- Fetches the settings and shows the source lists.
--- @param onReturnCallback fun(): nil
function SourceListsListing:fetchAndShow(onReturnCallback)
  local settings_response = Backend.getSettings()
  if settings_response.type == "ERROR" then
    ErrorDialog:show(settings_response.message)
    return
  end
  local status_response = Backend.listSourceCatalogs()
  if status_response.type == "ERROR" then
    ErrorDialog:show(status_response.message)
    return
  end
  local catalog_statuses = {}
  for _, status in ipairs(status_response.body) do
    catalog_statuses[status.list_id] = status
  end

  local ui = SourceListsListing:new {
    settings = settings_response.body,
    catalog_statuses = catalog_statuses,
    on_return_callback = onReturnCallback,
    covers_fullscreen = true,
  }
  ui.on_return_callback = onReturnCallback
  UIManager:show(ui)
  Testing:emitEvent("source_lists_listing_shown")
end

return SourceListsListing
