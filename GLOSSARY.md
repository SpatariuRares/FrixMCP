# FrixMCP

MCP server and local dashboard that let an AI and a person observe and govern a home FRITZ!Box with the same permission model.

## People and identity

**Operator**:
The human who uses FrixMCP; the only one who can confirm a Dangerous operation.
_Avoid_: user (on its own)

**Agent**:
The AI that uses FrixMCP over MCP; it can propose Dangerous operations but never confirm them.
_Avoid_: Claude, the model, mcp (as the name of an actor)

**Initiator**:
Whoever requested a write: the Agent or the Operator.
_Avoid_: origin, mcp/web

**Router Account**:
The FRITZ!OS user that FrixMCP authenticates as on the router.
_Avoid_: user (on its own), admin

**Control Host**:
The Host that runs FrixMCP and its dashboard; FrixMCP must never cut it off the network.
_Avoid_: the Mac, local machine, localhost

## Network

**Host**:
Any device the router knows in its host list, mesh nodes included; its identity is the MAC, never the IP.
_Avoid_: device, client (on their own)

**SmartHome Device**:
A home automation device managed by the router and identified by an AIN (plug, thermostat, sensor).
_Avoid_: device (on its own)

**Wi-Fi Network**:
One of the router's three Wi-Fi networks: Main 2.4 GHz, Main 5 GHz, Guest.
_Avoid_: band (meaning a network), WLAN, instance

**Band**:
The radio frequency a Wi-Fi Network transmits on (2.4 GHz or 5 GHz).
_Avoid_: network (meaning a frequency)

## Permissions

**Level**:
The risk grade of an exposed operation: Read, Write or Dangerous.
_Avoid_: level X, excluded (not a Level)

**Classification**:
The decision made for each router action: exposed with a Level, or Forbidden.

**Forbidden**:
Classification of a router action that FrixMCP cannot execute under any circumstances, because it is absent from the program.
_Avoid_: excluded, level X, disabled

**Confirmation**:
The consent the Operator gives to a single Dangerous operation, on a channel the AI can neither read nor operate.
_Avoid_: confirm token, confirmation in chat

**Preview**:
The description of a write's effect, computed without executing it; it is what the Operator sees before a Confirmation.
_Avoid_: dry run (that is the parameter name, not the concept), summary

**Lockout**:
The Control Host's loss of access to the router that does not resolve on its own; FrixMCP always prevents it, at any Level.
_Avoid_: disconnection (a reboot disconnects but is not a Lockout)

## Traceability

**Audit Entry**:
The record of an executed write: who requested it, what, the state before and the state after.
_Avoid_: log (the log is the router's)

**Reversibility**:
Declared property of every write operation: Reversible if reapplying the previous state undoes its effect, Irreversible otherwise.
_Avoid_: rollback, undo
