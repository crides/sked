sched:add_handler(":.*:", function(log)
    print("type:", log.type)
    print("attrs:", log.attrs)
end)

sched:new_log(":init:")
repl()
