world = {}

local wrap_trigger_callback = function(callback)
    return function(name, line, wildcards, styles)
        return coroutine.wrap(callback)(name, line, wildcards, styles)
    end
end

local create_trigger = function(args)
    assert(args.pattern, "pattern of trigger cannot be empty")
    assert(type(args.callback) == "function", "callback of trigger must be function")
    assert(type(args.flags) == "number", "flags of trigger must be number")
    if not args.name then
        args.name = "trigger-" .. GetUniqueID()
    end
    if not args.group then
        args.group = "default"
    end
    if not args.match_lines then
        args.match_lines = 1
    end

    local callback = wrap_trigger_callback(args.callback)
    CreateTrigger(args.name, args.group, args.pattern, args.flags, args.match_lines, callback)
end

-- 创建触发器
-- 参数：
-- name：名称，不传值则自动生成唯一id
-- group：组名，默认为"default"
-- pattern：正则表达式，不可为空，如果多行，需使用\r\n作为行分隔符
-- match_lines：匹配行数，默认为1，当且仅当>1时多行匹配生效
-- func：匹配成功的回调函数，不可为空，函数内部可使用协程相关指令，
--       即，该函数传入后将包装为协程进行调用。
--       该函数接受4个参数，按顺序为：
--       1) name，该触发器名称。
--       2) line，文本，若多行匹配，则为多行文本，以\r\n分隔。
--       3) wildcards，正则捕获序列，实现为lua table，可使用数字
--          或字符串下标进行取值。
--       4) styles，文本格式，用于判断文本的颜色和特殊格式，仅支
--          持单行模式，多行模式下为空。
function world.create_trigger(args)
    args.flags = trigger_flag.Enabled
    create_trigger(args)
end

-- 创建临时触发器（触发一次后自动删除）
function world.create_oneshot_trigger(args)
    -- 不使用bitOp库，而直接使用数字相加设置bitflag
    args.flags = trigger_flag.Enabled + trigger_flag.OneShot
    create_trigger(args)
end

-- 删除触发器
-- 参数name: 名称，不可为空
function world.delete_trigger(name)
    assert(name, "name of trigger cannot be empty")
    DeleteTrigger(name)
end

-- 开启/禁用触发器组
-- 参数：
-- 1. name，组名，不可为空
-- 2. enabled，true开启/false禁用，默认为true
function world.enable_trigger_group(group, enabled)
    enabled = enabled or true
    EnableTriggerGroup(group, enabled)
end

local wrap_alias_callback = function(callback)
    return function(name, line, wildcards)
        return coroutine.wrap(callback)(name, line, wildcards)
    end
end

local create_alias = function(args)
    assert(args.pattern, "pattern of alias cannot be empty")
    assert(type(args.callback) == "function", "callback of alias must be function")
    assert(type(args.flags) == "number", "flags of alias must be number")
    if not args.name then
        args.name = "alias-" .. GetUniqueID()
    end
    if not args.group then
        args.group = "default"
    end
    local callback = wrap_alias_callback(args.callback)
    CreateAlias(args.name, args.group, args.pattern, args.flags, callback)
end

-- 创建别名
-- 参数：
-- name：名称，不传值则自动生成唯一id
-- group：组名，默认为"default"
-- pattern：正则表达式，不可为空，只支持单行匹配
-- func：匹配成功的回调函数，不可为空，函数内部可使用协程相关指令，
--       即，该函数传入后将包装为协程进行调用。
--       该函数接受4个参数，按顺序为：
--       1) name，该触发器名称。
--       2) line，文本，若多行匹配，则为多行文本，以\r\n分隔。
--       3) wildcards，正则捕获序列，实现为lua table，可使用数字
--          或字符串下标进行取值。
function world.create_alias(args)
    args.flags = alias_flag.Enabled
    create_alias(args)
end

-- 删除别名
-- 参数name: 名称，不可为空
function world.delete_alias(name)
    assert(name, "name of alias cannot be empty")
    DeleteAlias(name)
end

-- 开启/禁用别名组
-- 参数：
-- 1. name，组名，不可为空
-- 2. enabled，true开启/false禁用，默认为true
function world.enable_alias_group(group, enabled)
    enabled = enabled or true
    EnableAliasGroup(group, enabled)
end

local wrap_timer_callback = function(callback)
    return function()
        return coroutine.wrap(callback)()
    end
end

local create_timer = function(args)
    assert(type(args.callback) == "function", "callback of alias must be a function")
    assert(type(args.flags) == "number", "flags of alias must be number")
    if not args.name then
        args.name = "alias-" .. GetUniqueID()
    end
    if not args.group then
        args.group = "default"
    end
    local callback = wrap_timer_callback(args.callback)
    CreateTimer(args.name, args.group, args.tick_in_millis, args.flags, callback)
end

-- 创建定时器
-- 参数：
-- name：名称，不传值则自动生成唯一id
-- group：组名，默认为"default"
-- tick_time：周期时间，单位为秒
-- func：匹配成功的回调函数，不可为空，函数内部可使用协程相关指令，
--       即，该函数传入后将包装为协程进行调用。
--       该函数接受4个参数，按顺序为：
--       1) name，该触发器名称。
--       2) line，文本，若多行匹配，则为多行文本，以\r\n分隔。
--       3) wildcards，正则捕获序列，实现为lua table，可使用数字
--          或字符串下标进行取值。
function world.create_timer(args)
    assert(type(args.tick_time) == "number", "tick time of timer must be number")
    args.tick_in_millis = args.tick_time * 1000
    args.flags = timer_flag.Enabled
    create_timer(args)
end

function world.create_oneshot_timer(args)
    assert(type(args.tick_time) == "number", "tick time of timer must be number")
    args.tick_in_millis = args.tick_time * 1000
    args.flags = timer_flag.Enabled + timer_flag.OneShot
    create_timer(args)
end

-- 删除定时器
-- 参数name: 名称，不可为空
function world.delete_timer(name)
    assert(name, "name of timer cannot be empty")
    DeleteTimer(name)
end

-- 开启/禁用定时器组
-- 参数：
-- 1. name，组名，不可为空
-- 2. enabled，true开启/false禁用，默认为true
function world.enable_timer_group(group, enabled)
    enabled = enabled or true
    EnableTimerGroup(group, enabled)
end

-- 等待指定时间
-- 该方法调用必须在协程中
function world.wait_time(timeout)
    assert(type(timeout) == "number", "timeout should be number")
    local thread = assert(coroutine.running(), "wait_time must be called in coroutine")
    world.create_oneshot_timer{
        group="WaitTime",
        tick_time=timeout,
        callback=function()
            local ok, err = coroutine.resume(thread)
            if not ok then
                error(err)
            end
        end
    }
    return coroutine.yield()
end

-- 等待某一行文本触发，超时时间可选
-- 该方法调用必须在协程中
-- 调用方可根据返回值个数判断是因为timeout还是因为触发而执行
function world.wait_regexp(pattern, timeout)
    assert(type(pattern) == "string", "pattern should be string")
    if timeout then
        assert(type(timeout) == "number", "timeout must be number")
    end
    -- 创建临时定时器用于在timeout后删除触发
    local thread = assert(coroutine.running(), "wait_regexp must be called in coroutine")
    local trigger_name = "WaitRegexp-" .. GetUniqueID()
    local timer_name
    if timeout then
        timer_name = "Timeout-" .. GetUniqueID()
        world.create_oneshot_timer{
            name=timer_name,
            group="WaitRegexp",
            tick_time=timeout,
            callback=function()
                -- timer执行时，将trigger删除
                world.delete_trigger(trigger_name)
                local ok, err = coroutine.resume(thread)
                if not ok then
                    error(err)
                end
            end 
        } 
    end
    world.create_oneshot_trigger{
        name=trigger_name,
        group="WaitRegexp",
        pattern=pattern,
        callback=function(name, line, wildcards, styles)
            -- trigger执行时，将timer删除
            if timer_name then
                world.delete_timer(timer_name)
            end
            local ok, err = coroutine.resume(thread, name, line, wildcards, styles)
            if not ok then
                error(err)
            end
        end
    }
    return coroutine.yield()
end
