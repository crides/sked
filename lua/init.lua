event_handlers = {}
function add_handler(pat, handler)
    table.insert(event_handlers, {pat, handler})
    pprint(event_handlers)
end
function sched.log.new(typ, map)
    sched.log._new(typ, map)
    for _, handler in ipairs(event_handlers) do
        if string.match(typ, handler[1]) then
            handler[2](typ, map)
        end
    end
end

add_handler(":.*:", function(typ, map)
    print("type:", typ)
    print("attrs:", map)
end)

sched.log.new(":init:")
repl()
