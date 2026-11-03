// Which of the phone's addresses the TV can reach it at. The Cast SDK doesn't
// always say where the TV is, so this goes by what the phone itself is on.
package io.github.okaza03.valo.cast

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import java.net.Inet4Address
import java.net.InetAddress
import java.net.NetworkInterface

/** One of the phone's addresses, on a network `prefix` bits long. `home`: Android says it's Wi-Fi or Ethernet. */
data class LocalAddress(val iface: String, val ip: InetAddress, val prefix: Int, val home: Boolean = false)

/** Interfaces that lead to the home network: Wi-Fi, a USB or Ethernet adapter, the phone's own hotspot. */
private val LOCAL = Regex("^(wlan|swlan|ap|eth|usb|rndis).*")
/** Interfaces that never do: mobile data and VPN tunnels. */
private val AWAY = Regex("^(rmnet|ccmni|pdp|seth|v4-|tun|ppp|ipsec|wg|dummy|lo).*")

/** Both addresses are on the same network, `prefix` bits long. */
private fun sameNetwork(a: InetAddress, b: InetAddress, prefix: Int): Boolean {
    val (x, y) = a.address to b.address
    if (x.size != y.size || prefix !in 1..x.size * 8) return false
    for (bit in 0 until prefix) {
        val mask = 0x80 shr (bit % 8)
        if (x[bit / 8].toInt() and mask != y[bit / 8].toInt() and mask) return false
    }
    return true
}

/**
 * The address to give the TV: one on the TV's own network when we know it,
 * else Wi-Fi or Ethernet by what Android says, then by name, then any private
 * address that isn't mobile data or a VPN tunnel.
 */
fun pickAddress(tv: InetAddress?, mine: List<LocalAddress>): InetAddress? {
    val usable = mine.filter { it.ip is Inet4Address && !it.ip.isLoopbackAddress && !it.ip.isLinkLocalAddress }
    return (usable.firstOrNull { tv != null && sameNetwork(it.ip, tv, it.prefix) }
        ?: usable.firstOrNull { it.home }
        ?: usable.firstOrNull { LOCAL.matches(it.iface) }
        ?: usable.firstOrNull { it.ip.isSiteLocalAddress && !AWAY.matches(it.iface) })?.ip
}

/** What the address came from, for when none fits: "TV 192.168.1.5; wlan0 192.168.1.9/24". */
fun addressReport(tv: InetAddress?, mine: List<LocalAddress>): String =
    (listOf("TV ${tv?.hostAddress ?: "unknown"}") + mine.distinctBy { it.iface to it.ip }.map { "${it.iface} ${it.ip.hostAddress}/${it.prefix}" }).joinToString("; ")

/** Every address the phone has: what Android says about its networks, then its interfaces. */
fun localAddresses(ctx: Context): List<LocalAddress> {
    val cm = ctx.getSystemService(ConnectivityManager::class.java)
    @Suppress("DEPRECATION")
    val known = runCatching {
        cm.allNetworks.toList().flatMap { n ->
            val caps = cm.getNetworkCapabilities(n)
            val vpn = caps?.hasTransport(NetworkCapabilities.TRANSPORT_VPN) == true
            val home = !vpn && (caps?.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) == true || caps?.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) == true)
            val p = cm.getLinkProperties(n) ?: return@flatMap emptyList()
            p.linkAddresses.map { LocalAddress(p.interfaceName.orEmpty(), it.address, it.prefixLength, home) }
        }
    }.getOrDefault(emptyList())
    val found = runCatching {
        NetworkInterface.getNetworkInterfaces().toList().filter { it.isUp }.flatMap { n ->
            n.interfaceAddresses.map { LocalAddress(n.name, it.address, it.networkPrefixLength.toInt()) }
        }
    }.getOrDefault(emptyList())
    // Android's list first: it knows which is Wi-Fi
    return (known + found).distinctBy { it.iface to it.ip }
}
