package io.github.okaza03.valo.cast

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import java.net.InetAddress

class LocalAddressTest {
    private fun ip(s: String): InetAddress = InetAddress.getByName(s)
    private val wifi = LocalAddress("wlan0", ip("192.168.68.40"), 22)
    private val mobile = LocalAddress("rmnet_data0", ip("10.120.3.9"), 30)
    private val vpn = LocalAddress("tun0", ip("10.8.0.2"), 24)

    @Test
    fun theTvsOwnNetworkWins() {
        val second = LocalAddress("eth0", ip("172.16.0.5"), 24)
        assertEquals(second.ip, pickAddress(ip("172.16.0.9"), listOf(wifi, second)))
    }

    @Test
    fun aWideNetworkCountsToo() {
        // 192.168.68.0/22 reaches up to .71
        assertEquals(wifi.ip, pickAddress(ip("192.168.70.3"), listOf(mobile, wifi)))
    }

    @Test
    fun wifiWithoutKnowingWhereTheTvIs() {
        assertEquals(wifi.ip, pickAddress(null, listOf(mobile, vpn, wifi)))
    }

    @Test
    fun anAddressThatCantBeComparedStillFindsWifi() {
        // what the old code failed on: an IPv6 link-local TV
        assertEquals(wifi.ip, pickAddress(ip("fe80::1a2b:3c4d:5e6f:7a8b"), listOf(mobile, wifi)))
    }

    @Test
    fun aVpnOrMobileDataIsNeverTheAnswer() {
        assertEquals(wifi.ip, pickAddress(ip("192.168.68.20"), listOf(vpn, mobile, wifi)))
        assertNull(pickAddress(ip("192.168.68.20"), listOf(vpn, mobile)))
    }

    @Test
    fun ipv6AndLoopbackAreLeftOut() {
        val v6 = LocalAddress("wlan0", ip("fe80::1"), 64)
        val lo = LocalAddress("lo", ip("127.0.0.1"), 8)
        assertNull(pickAddress(null, listOf(v6, lo)))
    }

    @Test
    fun androidsWordBeatsTheName() {
        val named = LocalAddress("wifi0", ip("192.168.1.20"), 24, home = true)
        assertEquals(named.ip, pickAddress(null, listOf(mobile, vpn, named)))
    }

    @Test
    fun anUnknownInterfaceWithAPrivateAddressStillCounts() {
        val odd = LocalAddress("mlan0", ip("192.168.1.20"), 24)
        assertEquals(odd.ip, pickAddress(null, listOf(mobile, vpn, odd)))
    }

    @Test
    fun theReportSaysWhatWasFound() {
        assertEquals("TV 192.168.1.5; rmnet_data0 10.120.3.9/30", addressReport(ip("192.168.1.5"), listOf(mobile)))
        assertEquals("TV unknown", addressReport(null, emptyList()))
    }

    @Test
    fun theHotspotCounts() {
        val ap = LocalAddress("swlan0", ip("192.168.43.1"), 24)
        assertEquals(ap.ip, pickAddress(null, listOf(mobile, ap)))
    }
}
