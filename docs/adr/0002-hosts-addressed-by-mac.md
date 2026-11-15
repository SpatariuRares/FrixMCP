# Hosts are always addressed by MAC

Every tool that acts on a Host accepts only the MAC; FrixMCP resolves MAC → IP at call time, even for router actions that work by IP (HostFilter, priority). A DHCP-assigned IP can move to another Host between the read and the write, and blocking the internet "for the iPad's IP" risks hitting the smart TV that received that lease. We accept that a phone with a private Wi-Fi address shows up as several distinct Hosts: that is what the router sees.
