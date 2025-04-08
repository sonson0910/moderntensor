--gen coldkey
mtcli w create-coldkey --name kickoff --base-dir "moderntensor"
--gen hotkey
mtcli w generate-hotkey --coldkey kickoff --hotkey-name hk1 --base-dir "moderntensor"
pass 123456