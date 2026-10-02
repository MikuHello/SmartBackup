@ECHO OFF
:: ============================================================================
:: 7-Zip 智能备份系统 v1.0.0
::
:: 作者: MikuHello
:: ============================================================================
::
:: ####################### 开发者经验总结 (v1.0.0) ######################
::
:: 本总结记录了开发此脚本时遇到的核心陷阱与最佳实践，以确保其长期稳定。
::
:: --- 核心陷阱规避 ---
::
:: 1. [陷阱：命令解析与延迟扩展]
::    - 症状: ECHO输出含颜色代码或特殊字符(如 `[]`)的变量时报错。
::    - 原理: CMD的标准解析模式会在“执行前”替换`%变量%`，导致特殊
::          字符被错误解析。
::    - 方案: 启用延迟扩展(`SETLOCAL ENABLEDELAYEDEXPANSION`)，并在
::          ECHO时使用`!变量!`输出。变量在“执行时”才被展开，完美绕过陷阱。
::
:: 2. [陷阱：与PowerShell的进程间通信]
::    - 症状: 在`FOR /F`循环中调用内联PowerShell获取动态输出时不稳定。
::    - 原理: CMD的引号和转义规则在处理`FOR /F`内部的复杂命令时极其
::          脆弱，容易失败。
::    - 方案: 采用“临时环境变量”作为通信桥梁。批处理将动态数据存入
::          环境变量(`SET PS_VAR=...`)，PowerShell再从环境(`$env:PS_VAR`)中
::          读取。此方法将数据与指令分离，最为稳定可靠。
::
:: 3. [陷阱：特殊字符的转义与上下文]
::    - 症状: 脚本意外创建文件、日志错乱或闪退。
::    - 原理: 重定向符(`>`,`<`,`|`)的转义效果依赖其上下文。将已转义
::          的字符存入变量或在`()`代码块中使用，都可能因二次解析而失效或崩溃。
::    - 方案: 永远只在最终输出的`ECHO`命令中“直接”使用并转义它，
::          并避免将该ECHO命令置于非必须的括号代码块中。
::
:: --- 开发规范与技巧 ---
::
:: 4. [技巧：动态ANSI颜色定义]
::    - 需求: 在不硬编码的前提下，让脚本在不同系统中都能彩色输出。
::    - 原理: 硬编码特殊的ESC控制字符会引发文件编码问题和可移植性下降。
::    - 方案: 利用`FOR /F`捕获`prompt $E`命令的输出。该命令会打印出
::          当前终端的ESC字符，从而动态、无错地将其存入变量，作为所有
::          ANSI颜色代码的基础。
::
:: 5. [规范：版本号与日志策略]
::    - Dev: 开发版 (如 v-Dev x.x.x)。用于添加新功能和重构。
::    - Releases: 正式版 (如 v1.0.0)。稳定、可靠，面向最终用户，不含更新日志。
::    - DEBUG: 修复版。为修复特定BUG而发布，头部会保留详细更新日志。
::
:: ##########################################################################


:: --- 初始设置 ---
CHCP 65001 > NUL
TITLE 7-Zip 智能备份系统 v1.0.0

:: ============================================================================
:: 1. 用户配置区
:: ============================================================================
SET "sevenZip=C:\Program Files\7-Zip\7z.exe"
SET "backupFolder=D:\Backup"

:: ============================================================================
:: 2. 颜色和样式定义
:: ============================================================================
FOR /F "tokens=1 delims= " %%A IN ('"prompt $E & echo on & for %%B in (1) do rem"') DO (SET "ESC=%%A")
SET "C_RESET=%ESC%[0m"
SET "C_RED=%ESC%[91m"
SET "C_GREEN=%ESC%[92m"
SET "C_YELLOW=%ESC%[93m"
SET "C_CYAN=%ESC%[96m"
SET "C_WHITE=%ESC%[97m"
SET "C_GRAY=%ESC%[90m"

:: ============================================================================
:: 3. 主菜单
:: ============================================================================
:main
    SETLOCAL ENABLEDELAYEDEXPANSION
    CLS
    
    SET "menu_item2=    %C_CYAN%[2]%C_RESET% 备份指定文件夹 %C_GRAY%（将单个文件夹打包）%C_RESET%"
    SET "menu_item3=    %C_CYAN%[3]%C_RESET% 合并备份 %C_GRAY%（将多个文件或文件夹打包）%C_RESET%"

    ECHO.
    ECHO %C_CYAN%====================================================================%C_RESET%
    ECHO                     %C_WHITE%7-Zip 智能备份系统 v1.0.0%C_RESET%
    ECHO %C_CYAN%====================================================================%C_RESET%
    ECHO.
    ECHO   %C_WHITE%[菜单选项]%C_RESET%
    ECHO     %C_CYAN%[1]%C_RESET% 备份当前文件夹
    ECHO !menu_item2!
    ECHO !menu_item3!
    ECHO     %C_CYAN%[4]%C_RESET% 退出
    ECHO.
    ECHO %C_CYAN%--------------------------------------------------------------------%C_RESET%
    ECHO.
    
    SET "choice="
    SET /P "choice=%C_CYAN%>> %C_RESET%请输入选项 [1-4] 并按回车: "

    IF "!choice!"=="1" GOTO backup_current
    IF "!choice!"=="2" GOTO backup_specific
    IF "!choice!"=="3" GOTO backup_combined
    IF "!choice!"=="4" GOTO exit_script

    CALL :log "ERROR" "无效输入，请重新选择。"
    PAUSE
GOTO main

:: ============================================================================
:: 4. 备份模式实现
:: ============================================================================
:backup_current
    ENDLOCAL
    SET "sourceFolder=%cd%"
    SET "backupType=当前文件夹备份"
GOTO confirm_settings

:backup_specific
    ENDLOCAL
    ECHO.
    CALL :log "INFO" "请输入要完整备份的单个文件夹的路径。"
    ECHO.
    SET /P "sourceFolder=%C_CYAN%>> %C_RESET%请输入文件夹路径: "
    IF NOT EXIST "%sourceFolder%" (
        CALL :log "ERROR" "文件夹 "%sourceFolder%" 不存在，请重新输入。"
        PAUSE
        GOTO main
    )
    SET "backupType=指定文件夹备份"
GOTO confirm_settings

:backup_combined
    ENDLOCAL
    SET "backupType=合并备份"
    ECHO.
    CALL :log "INFO" "合并备份模式：将多个文件/文件夹打包成一个压缩包。"
    ECHO.
    SET "outputName="
    SET /P "outputName=%C_CYAN%>> %C_RESET%请输入压缩包的名称 (不含.7z): "
    IF "%outputName%"=="" (
        CALL :log "WARN" "名称不能为空，操作已取消。"
        TIMEOUT /T 2 /NOBREAK > NUL
        GOTO main
    )

    SET "pathList="
    SET "pathCount=0"
:collect_paths_loop
    CLS
    ECHO.
    ECHO %C_CYAN%====================================================================%C_RESET%
    ECHO                       %C_WHITE%添加要备份的文件或文件夹%C_RESET%
    ECHO %C_CYAN%====================================================================%C_RESET%
    ECHO.
    ECHO   %C_WHITE%当前已添加 %C_YELLOW%%pathCount%%C_WHITE% 个项目到压缩包 %C_YELLOW%%outputName%.7z%C_RESET%
    IF DEFINED pathList (
        ECHO   %C_GRAY%列表: %pathList% %C_RESET%
    )
    ECHO.
    ECHO   %C_GRAY%输入一个完整路径后按回车，输入 %C_YELLOW%q%C_GRAY% 完成添加。%C_RESET%
    ECHO.
    
    SET "currentPath="
    SET /P "currentPath=%C_CYAN%>> %C_RESET%请输入路径: "

    IF /I "%currentPath%"=="q" GOTO confirm_combined_settings
    
    IF "%currentPath%"=="" GOTO collect_paths_loop

    IF NOT EXIST "%currentPath%" (
        CALL :log "ERROR" "路径 "%currentPath%" 无效，请重新输入。"
        PAUSE > NUL
        GOTO collect_paths_loop
    )

    IF DEFINED pathList (
        SET "pathList=%pathList% "%currentPath%""
    ) ELSE (
        SET "pathList="%currentPath%""
    )
    SET /A pathCount+=1
    GOTO collect_paths_loop

:: ============================================================================
:: 5. 备份设置确认
:: ============================================================================
:confirm_settings
    CLS
    ECHO.
    ECHO %C_CYAN%====================================================================%C_RESET%
    ECHO                               %C_WHITE%确认备份设置%C_RESET%
    ECHO %C_CYAN%====================================================================%C_RESET%
    ECHO.
    ECHO   %C_CYAN%[基本信息]%C_RESET%
    ECHO     %C_WHITE%备份模式 : %C_YELLOW%%backupType%%C_RESET%
    ECHO     %C_WHITE%源文件夹 : %C_YELLOW%%sourceFolder%%C_RESET%
    ECHO     %C_WHITE%备份目录 : %C_YELLOW%%backupFolder%%C_RESET%
    ECHO.
    ECHO   %C_CYAN%[压缩配置]%C_RESET%
    ECHO     %C_GRAY%格式: 7z      级别: 9 (极限)    方法: LZMA2%C_RESET%
    ECHO     %C_GRAY%字典: 256MB   固实块: 16GB      线程: 自动%C_RESET%
    ECHO     %C_GRAY%排除: desktop.ini, Thumbs.db (系统隐藏文件)%C_RESET%
    ECHO.
    ECHO %C_CYAN%--------------------------------------------------------------------%C_RESET%
    ECHO.
    
    SET "confirm="
    SET /P "confirm=%C_YELLOW%>> %C_RESET%确认开始备份吗？ (Y/N): "
    IF /I "%confirm%" NEQ "Y" (
        CALL :log "WARN" "操作已取消，正在返回主菜单..."
        TIMEOUT /T 2 /NOBREAK > NUL
        GOTO main
    )
GOTO start_backup

:confirm_combined_settings
    IF NOT DEFINED pathList (
        CALL :log "WARN" "未添加任何备份项目，操作已取消。"
        TIMEOUT /T 2 /NOBREAK > NUL
        GOTO main
    )
    CLS
    ECHO.
    ECHO %C_CYAN%====================================================================%C_RESET%
    ECHO                             %C_WHITE%确认合并备份设置%C_RESET%
    ECHO %C_CYAN%====================================================================%C_RESET%
    ECHO.
    ECHO   %C_CYAN%[基本信息]%C_RESET%
    ECHO     %C_WHITE%备份模式 : %C_YELLOW%%backupType%%C_RESET%
    ECHO     %C_WHITE%输出名称 : %C_YELLOW%%outputName%%C_RESET%.7z
    ECHO     %C_WHITE%备份目录 : %C_YELLOW%%backupFolder%%C_RESET%
    ECHO.
    ECHO   %C_CYAN%[备份项目列表 (%pathCount% 个)]%C_RESET%
    ECHO     %C_YELLOW%%pathList%%C_RESET%
    ECHO.
    ECHO   %C_CYAN%[压缩配置]%C_RESET%
    ECHO     %C_GRAY%格式: 7z      级别: 9 (极限)    方法: LZMA2%C_RESET%
    ECHO     %C_GRAY%字典: 256MB   固实块: 16GB      线程: 自动%C_RESET%
    ECHO     %C_GRAY%排除: desktop.ini, Thumbs.db (系统隐藏文件)%C_RESET%
    ECHO.
    ECHO %C_CYAN%--------------------------------------------------------------------%C_RESET%
    ECHO.
    SET "confirm="
    SET /P "confirm=%C_YELLOW%>> %C_RESET%确认开始备份吗？ (Y/N): "
    IF /I "%confirm%" NEQ "Y" (
        CALL :log "WARN" "操作已取消，正在返回主菜单..."
        TIMEOUT /T 2 /NOBREAK > NUL
        GOTO main
    )
GOTO start_backup

:: ============================================================================
:: 6. 执行备份核心
:: ============================================================================
:start_backup
    SET "zip_switches=-t7z -mx9 -m0=LZMA2 -md=256m -ms=16g -mmt=on -xr!desktop.ini -xr!Thumbs.db -y -sccUTF-8"
    CLS
    ECHO.
    CALL :log "INFO" "环境检查..."
    
    IF NOT EXIST "%sevenZip%" (
        CALL :log "ERROR" "未找到 7-Zip 主程序，请检查配置中的路径。"
        CALL :log "INFO" "期望路径: %sevenZip%"
        PAUSE
        GOTO main
    )

    IF NOT EXIST "%backupFolder%" (
        CALL :log "INFO" "备份目录不存在，正在尝试创建: %backupFolder%"
        MKDIR "%backupFolder%"
        IF %ERRORLEVEL% NEQ 0 (
            CALL :log "ERROR" "无法创建备份目录，请检查权限或路径。"
            PAUSE
            GOTO main
        )
        CALL :log "SUCCESS" "目录创建成功！"
    )
    ECHO.
    
    FOR /F "tokens=1,2 delims=," %%i IN ('powershell -Command "(Get-Date -Format 'yyyyMMdd') + ',' + (Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"') DO (
        SET "today=%%i"
        SET "startTime=%%j"
    )

    CALL :log "INFO" "任务开始于: %startTime%"
    ECHO.

    IF "%backupType%"=="合并备份" GOTO backup_combined_execute
    GOTO backup_single_folder_execute

:backup_single_folder_execute
    FOR %%I IN ("%sourceFolder%") DO SET "folderName=%%~nxI"
    SET "outputFile=%backupFolder%\%folderName%_%today%.7z"
    CALL :log "INFO" "输出文件: %outputFile%"
    ECHO.
    CALL :log "PROGRESS" "压缩过程已开始，请稍候..."
    ECHO %C_GRAY%--------------------------------------------------------------------%C_RESET%
    "%sevenZip%" a %zip_switches% "%outputFile%" "%sourceFolder%"
    SET "exitCode=%ERRORLEVEL%"
    ECHO %C_GRAY%--------------------------------------------------------------------%C_RESET%
GOTO check_result

:backup_combined_execute
    SET "outputFile=%backupFolder%\%outputName%_%today%.7z"
    CALL :log "INFO" "输出文件: %outputFile%"
    ECHO.
    CALL :log "PROGRESS" "压缩过程已开始，请稍候..."
    ECHO %C_GRAY%--------------------------------------------------------------------%C_RESET%
    "%sevenZip%" a %zip_switches% "%outputFile%" %pathList%
    SET "exitCode=%ERRORLEVEL%"
    ECHO %C_GRAY%--------------------------------------------------------------------%C_RESET%
GOTO check_result

:: ============================================================================
:: 7. 任务结果报告
:: ============================================================================
:check_result
    FOR /F "delims=" %%i IN ('powershell -Command "Get-Date -Format 'yyyy-MM-dd HH:mm:ss'"') DO SET "endTime=%%i"
    ECHO.

    IF %exitCode% EQU 0 (
        SETLOCAL ENABLEDELAYEDEXPANSION
        IF EXIST "!outputFile!" (
            SET "PS_TARGET_FILE_PATH=!outputFile!"
            
            FOR /F "tokens=1,2 delims=," %%a IN ('powershell -NoProfile -C "$f=Get-Item -LiteralPath $env:PS_TARGET_FILE_PATH -ErrorAction SilentlyContinue; if($f){'{0:F2},{1:F0}' -f ($f.Length/1MB),($f.Length/1KB)}else{'0.00,0'}"') DO (
                SET "sizeMB=%%a"
                SET "sizeKB=%%b"
            )

        ) ELSE (
            SET "sizeMB=N/A"
            SET "sizeKB=N/A"
        )
        
        ECHO %C_GREEN%====================================================================%C_RESET%
        ECHO                                  %C_WHITE%备份成功%C_RESET%
        ECHO %C_GREEN%====================================================================%C_RESET%
        ECHO.
        ECHO   %C_GREEN%[任务报告]%C_RESET%
        ECHO     %C_WHITE%完成时间 : %C_YELLOW%!endTime!%C_RESET%
        ECHO     %C_WHITE%文件大小 : %C_YELLOW%!sizeMB! MB （!sizeKB! KB）%C_RESET%
        ECHO     %C_WHITE%压缩包   : %C_YELLOW%!outputFile!%C_RESET%
        ECHO.
        ECHO %C_GREEN%--------------------------------------------------------------------%C_RESET%
        ENDLOCAL
    ) ELSE (
        ECHO %C_RED%====================================================================%C_RESET%
        ECHO                                  %C_WHITE%备份失败%C_RESET%
        ECHO %C_RED%====================================================================%C_RESET%
        ECHO.
        ECHO   %C_RED%[错误报告]%C_RESET%
        ECHO     %C_WHITE%错误代码 : %C_YELLOW%%exitCode%%C_RESET%
        ECHO     %C_WHITE%可能原因 : %C_RESET%请检查 7-Zip 输出或以下常见问题:
        ECHO               %C_GRAY%- 磁盘空间是否已满%C_RESET%
        ECHO               %C_GRAY%- 是否有足够的文件读写权限%C_RESET%
        ECHO               %C_GRAY%- 源文件是否被其他程序锁定%C_RESET%
        ECHO.
        ECHO %C_RED%--------------------------------------------------------------------%C_RESET%
    )
    
:post_backup_menu
    SETLOCAL ENABLEDELAYEDEXPANSION
    SET "post_menu_1=  %C_CYAN%[1]%C_RESET% 返回主菜单"
    SET "post_menu_2=  %C_CYAN%[2]%C_RESET% 退出程序"
:post_backup_loop
    ECHO.
    ECHO %C_CYAN%[操作选项]%C_RESET%
    ECHO !post_menu_1!
    ECHO !post_menu_2!
    ECHO.
    SET "post_choice="
    SET /P "post_choice=%C_CYAN%>> %C_RESET%请输入选项 [1-2]: "
    IF "!post_choice!"=="1" GOTO main
    IF "!post_choice!"=="2" GOTO exit_script
    ECHO.
    CALL :log "ERROR" "无效输入，请按 1 或 2。"
    GOTO post_backup_loop

:: ============================================================================
:: 8. 退出脚本
:: ============================================================================
:exit_script
    ENDLOCAL
    CLS
    CALL :log "INFO" "感谢使用 7-Zip 智能备份系统！"
    TIMEOUT /T 2 /NOBREAK > NUL
EXIT /B 0

:: ============================================================================
:: 9. 辅助子程序
:: ============================================================================
:log
SET "log_type=%~1"
SET "log_message=%~2"

IF /I "%log_type%" == "INFO"     ECHO %C_CYAN% [i] %C_RESET%%log_message%
IF /I "%log_type%" == "SUCCESS"  ECHO %C_GREEN% [+] %C_RESET%%log_message%
IF /I "%log_type%" == "WARN"     ECHO %C_YELLOW% [!] %C_RESET%%log_message%
IF /I "%log_type%" == "ERROR"    ECHO %C_RED% [x] %C_RESET%%log_message%
IF /I "%log_type%" == "PROGRESS" ECHO %C_YELLOW% [^>] %C_RESET%%log_message%

GOTO :EOF
