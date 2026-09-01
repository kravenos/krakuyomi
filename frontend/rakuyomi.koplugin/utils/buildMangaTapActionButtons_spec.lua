---@diagnostic disable: undefined-global
local buildMangaTapActionButtons = require('utils/buildMangaTapActionButtons')

describe('buildMangaTapActionButtons', function()
  it('always exposes continue reading and chapter list in that order', function()
    local continue_callback = function() end
    local chapter_list_callback = function() end

    local buttons = buildMangaTapActionButtons(
      'Continue Reading',
      'List chapters',
      continue_callback,
      chapter_list_callback
    )

    assert.equal('Continue Reading', buttons[1][1].text)
    assert.equal(continue_callback, buttons[1][1].callback)
    assert.equal('List chapters', buttons[2][1].text)
    assert.equal(chapter_list_callback, buttons[2][1].callback)
  end)
end)
