package com.mpacer.core.live

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Le parcours planifie vient d'un fichier GPX ecrit par un autre outil : le
 * parser doit etre toleraant (ordre des attributs, espaces, espaces de noms)
 * tout en rejetant ce qui n'est pas une position.
 */
class PlannedRouteStoreTest {

    private val gpx = """
        <?xml version="1.0" encoding="UTF-8"?>
        <gpx version="1.1" creator="test">
          <trk>
            <name>Parilly</name>
            <trkseg>
              <trkpt lat="45.7740" lon="4.9160"><ele>180</ele></trkpt>
              <trkpt lon="4.9185" lat="45.7752"><ele>182</ele></trkpt>
              <trkpt lat="45.7764" lon="4.9210"><ele>181</ele></trkpt>
            </trkseg>
          </trk>
        </gpx>
    """.trimIndent()

    @Test
    fun aGpxTrackIsReadInOrderWhateverTheAttributeOrder() {
        val points = PlannedRouteStore.parseGpx(gpx)
        assertEquals(3, points.size)
        assertEquals(45.7740, points[0].first, 1e-9)
        assertEquals(4.9160, points[0].second, 1e-9)
        // Deuxieme point : longitude ecrite avant la latitude.
        assertEquals(45.7752, points[1].first, 1e-9)
        assertEquals(4.9185, points[1].second, 1e-9)
    }

    @Test
    fun aRouteWithoutTrackFallsBackToRoutePoints() {
        val rte = "<gpx><rte><rtept lat=\"48.0\" lon=\"2.0\"/><rtept lat=\"48.01\" lon=\"2.01\"/></rte></gpx>"
        val points = PlannedRouteStore.parseGpx(rte)
        assertEquals(2, points.size)
        assertEquals(48.01, points[1].first, 1e-9)
    }

    @Test
    fun unusableCoordinatesAreDropped() {
        val melange = "<gpx><trkpt lat=\"48.0\" lon=\"2.0\"/>" +
            "<trkpt lat=\"91.0\" lon=\"2.0\"/>" +
            "<trkpt lat=\"48.1\" lon=\"2.1\"/></gpx>"
        val points = PlannedRouteStore.parseGpx(melange)
        assertEquals(2, points.size)
        assertTrue(points.none { it.first > 90.0 })
    }

    @Test
    fun anEmptyFileGivesNoPoint() {
        assertTrue(PlannedRouteStore.parseGpx("<?xml version=\"1.0\"?><gpx/>").isEmpty())
        assertTrue(PlannedRouteStore.Route("vide", emptyList()).isEmpty)
    }

    @Test
    fun aLongRouteIsDecimatedButAlwaysKeepsTheEnd() {
        val points = (0..5_000).map { 45.0 + it * 1e-5 to 4.0 + it * 1e-5 }
        val reduit = PlannedRouteStore.decimer(points)
        assertTrue("points = " + reduit.size, reduit.size <= PlannedRouteStore.MAX_POINTS + 1)
        assertEquals(points.first(), reduit.first())
        assertEquals(points.last(), reduit.last())
        // Meme trace : la longueur reste comparable a quelques pour cent pres.
        val avant = PlannedRouteStore.distanceM(points)
        val apres = PlannedRouteStore.distanceM(reduit)
        assertTrue("avant=" + avant + " apres=" + apres, apres > avant * 0.97)
    }

    @Test
    fun theLengthIsTheSumOfTheSegments() {
        // Environ 1,11 km par degre de latitude.
        val points = listOf(48.0 to 2.0, 48.01 to 2.0)
        val distance = PlannedRouteStore.distanceM(points)
        assertTrue("distance = " + distance, distance in 1_000.0..1_200.0)
        assertEquals(0.0, PlannedRouteStore.distanceM(listOf(48.0 to 2.0)), 1e-9)
    }

    @Test
    fun thePayloadCarriesTheDeviceAndEveryPoint() {
        val route = PlannedRouteStore.Route("parilly", listOf(48.0 to 2.0, 48.01 to 2.01))
        val corps = PlannedRouteStore.payload("montre-a1b2", route)
        assertTrue(corps, corps.startsWith("{\"device\":\"montre-a1b2\",\"points\":["))
        assertTrue(corps, corps.contains("[48.000000,2.000000]"))
        assertTrue(corps, corps.contains("[48.010000,2.010000]"))
        assertTrue(corps, corps.endsWith("]}"))
        // Le nom d'appareil est assaini : aucun guillemet ne casse le JSON.
        val hostile = PlannedRouteStore.payload("a\"b\\c", route)
        assertTrue(hostile, hostile.contains("\"device\":\"abc\""))
    }
}
