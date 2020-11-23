world = {}

local wrap_trigger_callback = function(func)
    return function(name, line, wildcards, styles)
        return coroutine.wrap(func)(name, line, wildcards, styles)
    end
end

local create_trigger = function(args)
    assert(args.pattern, "pattern of trigger cannot be empty")
    assert(type(args.func) == "function", "func of trigger must be a function")
    assert(type(args.flags) == "number", "flags of trigger must be a function")
    if not args.name then
        args.name = "trigger-" .. GetUniqueID()
    end
    if not args.group then
        args.group = "default"
    end
    if not args.match_lines then
        args.match_lines = 1
    end

    local callback = wrap_trigger_callback(args.func)
    CreateTrigger(args.name, args.group, args.pattern, args.flags, args.match_lines, callback)
end

function world.create_trigger(args)
    args.flags = trigger_flag.Enabled
    create_trigger(args)
end

function world.create_oneshot_trigger(args)
    args.flags = bit.bor(0, trigger_flag.Enabled, trigger_flag.OneShot)
    create_trigger(args)
end

function world.delete_trigger(name)
    assert(name, "name of trigger cannot be empty")
    DeleteTrigger(name)
end

local wrap_alias_callback = function(func)
    return function(name, line, wildcards)
        return coroutine.wrap(func)(name, line, wildcards)
    end
end

local create_alias = function(args)
    assert(args.pattern, "pattern of trigger cannot be empty")
    assert(type(args.func) == "function", "func of trigger must be a function")
    assert(type(args.flags) == "number", "flags of trigger must be a function")
    if not args.name then
        args.name = "trigger-" .. GetUniqueID()
    end
    if not args.group then
        args.group = "default"
    end
    local callback = wrap_alias_callback(args.func)
    CreateAlias(args.name, args.group, args.pattern, args.flags, callback)
end

function world.create_alias(args)
    args.flags = alias_flag.Enabled
    create_alias(args)
end

function world.delete_alias(name)
    DeleteAlias(name)
end
