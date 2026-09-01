---@diagnostic disable: undefined-global
local getPageTurnStyleChange = require('utils/getPageTurnStyleChange')

describe('getPageTurnStyleChange', function()
  it('switches KOReader page-scroll documents to continuous scrolling', function()
    assert.same({
      config_key = 'page_scroll',
      config_value = 1,
      event_name = 'SetScrollMode',
      event_value = true,
    }, getPageTurnStyleChange({ page_scroll = 0 }, 'scroll'))
  end)

  it('switches KOReader view-mode documents to paginated pages', function()
    assert.same({
      config_key = 'view_mode',
      config_value = 0,
      event_name = 'SetViewMode',
      event_value = 'page',
    }, getPageTurnStyleChange({ view_mode = 1 }, 'paginated'))
  end)

  it('does nothing when the requested style is already active', function()
    assert.is_nil(getPageTurnStyleChange({ page_scroll = 1 }, 'scroll'))
    assert.is_nil(getPageTurnStyleChange({ view_mode = 0 }, 'paginated'))
  end)

  it('ignores unset or invalid preferences', function()
    assert.is_nil(getPageTurnStyleChange({ page_scroll = 0 }, nil))
    assert.is_nil(getPageTurnStyleChange({ page_scroll = 0 }, 'animated'))
  end)

  it('does nothing until reader configuration exists', function()
    assert.is_nil(getPageTurnStyleChange(nil, 'scroll'))
  end)
end)
