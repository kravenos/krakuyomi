---@diagnostic disable: undefined-global
local applyReadingDirection = require('utils/applyReadingDirection')

local function makeView(inverse)
  return {
    inverse_reading_order = inverse,
    calls = {},
    onToggleReadingOrder = function(self, value)
      table.insert(self.calls, value)
      self.inverse_reading_order = value
    end,
  }
end

describe('applyReadingDirection', function()
  it('changes left-to-right reading to right-to-left', function()
    local view = makeView(false)

    assert.is_true(applyReadingDirection(view, 'rtl'))
    assert.same({ true }, view.calls)
  end)

  it('changes right-to-left reading to left-to-right', function()
    local view = makeView(true)

    assert.is_true(applyReadingDirection(view, 'ltr'))
    assert.same({ false }, view.calls)
  end)

  it('does not toggle a direction that is already active', function()
    local view = makeView(true)

    assert.is_false(applyReadingDirection(view, 'rtl'))
    assert.same({}, view.calls)
  end)

  it('ignores unset or invalid preferences', function()
    local view = makeView(false)

    assert.is_false(applyReadingDirection(view, nil))
    assert.is_false(applyReadingDirection(view, 'sideways'))
    assert.same({}, view.calls)
  end)

  it('does nothing until a compatible reader view exists', function()
    assert.is_false(applyReadingDirection(nil, 'rtl'))
    assert.is_false(applyReadingDirection({}, 'rtl'))
  end)
end)
