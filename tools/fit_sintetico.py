"""Generador de un FIT sintético coherente con un corredor de un .spl.

Inventa coordenadas para las balizas del recorrido del corredor (zona urbana alrededor de
Baltanás, Palencia; tramos de sprint de 50 a 250 m en línea recta) y genera un track a 1 Hz que
pasa por cada baliza exactamente en su hora de picada, convertida de hora local
(Europe/Madrid, con el horario de verano que toque en la fecha del .spl) a UTC. Añade altitud,
pulso y cadencia plausibles con ruido determinista (semilla fija) y dos "errores" conocidos:

  - rodeo: en un tramo (por defecto, el de split más largo) el corredor se desvía en U hacia un
    lado, de modo que la distancia recorrida es al menos 1,8 veces la línea recta;
  - parada: dentro de ese mismo tramo, 30 s con velocidad 0, lejos de las picadas.

Además, 60 s caminando antes de la salida y 60 s después de la meta, para que el track cubra la
carrera aunque se desplace unos segundos.

Escribe el FIT con un codificador mínimo propio (sin dependencias): cabecera de 14 bytes con
CRC, mensajes file_id, event, record, lap, session y activity, y CRC final. Guarda al lado un
`.truth.json` con la respuesta conocida (posiciones de las balizas, tramos, rodeo y parada).

Unidades en el FIT (perfil estándar): posición en semicírculos, altitud con escala 5 y offset
500, distancia en cm, velocidad en mm/s y cadencia en ciclos por minuto de un pie (como los
relojes Garmin en carrera; pasos por minuto = 2 x cadencia).

Uso:
  python3 tools/fit_sintetico.py fixtures/spl/baltanas-anon.spl --card 143 \\
      fixtures/fit/baltanas-sintetico.fit
  (escribe también fixtures/fit/baltanas-sintetico.truth.json)

El corredor se elige por su tarjeta SportIdent (0x84), que en un .spl anonimizado es el número
de orden del corredor y es única; el campo 0x80 del .spl se repite entre corredores.
"""
import argparse
import bisect
import datetime
import json
import math
import os
import random
import struct
import sys
from zoneinfo import ZoneInfo

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "reference"))
import winsplits_spl as ref  # noqa: E402

TIMEZONE = "Europe/Madrid"
UTC = datetime.timezone.utc
FIT_EPOCH = datetime.datetime(1989, 12, 31, tzinfo=UTC)
EARTH_RADIUS_M = 6371008.8
CENTER = (41.9380, -4.2490)  # Baltanás (Palencia)
DEFAULT_SEED = 20261003
MIN_LEG_M, MAX_LEG_M = 50.0, 250.0
MAX_NORMAL_RATIO = 1.5
AREA_RADIUS_M = 450.0
MIN_CONTROL_SEPARATION_M = 30.0
DETOUR_MIN_RATIO = 1.8
STOP_S = 30
PUNCH_MARGIN_S = 5  # la parada no puede estar en los 5 s posteriores (ni anteriores) a una picada
PRE_START_S = POST_FINISH_S = 60
WALK_SPEED = 1.0
RUN_SPEED = 3.7
DETOUR_RUN_SPEED = 3.1


# --- Tiempo -------------------------------------------------------------------------------------

def local_to_utc(date_iso, seconds_of_day, tz=TIMEZONE):
    """Hora local del .spl (segundos desde medianoche) -> instante UTC."""
    day = datetime.date.fromisoformat(date_iso)
    naive = datetime.datetime.combine(day, datetime.time()) + \
        datetime.timedelta(seconds=seconds_of_day)
    return naive.replace(tzinfo=ZoneInfo(tz)).astimezone(UTC)


def fit_timestamp(instant):
    return int((instant - FIT_EPOCH).total_seconds())


def iso_utc(instant):
    return instant.astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")


# --- Geometría (plano local en metros: x al este, y al norte) ---------------------------------

def to_latlon(x, y):
    lat0, lon0 = CENTER
    lat = lat0 + math.degrees(y / EARTH_RADIUS_M)
    lon = lon0 + math.degrees(x / (EARTH_RADIUS_M * math.cos(math.radians(lat0))))
    return lat, lon


def haversine_m(a, b):
    la1, lo1, la2, lo2 = map(math.radians, (a[0], a[1], b[0], b[1]))
    h = math.sin((la2 - la1) / 2) ** 2 + \
        math.cos(la1) * math.cos(la2) * math.sin((lo2 - lo1) / 2) ** 2
    return 2 * EARTH_RADIUS_M * math.asin(math.sqrt(h))


def to_semicircles(deg):
    return int(round(deg * 2 ** 31 / 180))


def from_semicircles(sc):
    return sc * 180 / 2 ** 31


def altitude_m(x, y):
    """Altitud del terreno: ligera pendiente hacia el norte y un cerro al nordeste."""
    hill = 24.0 * math.exp(-((x - 180.0) ** 2 + (y - 260.0) ** 2) / (2 * 170.0 ** 2))
    return 784.0 + 0.015 * y + hill


class Polyline:
    def __init__(self, points):
        self.points = points
        self.cum = [0.0]
        for (x0, y0), (x1, y1) in zip(points, points[1:]):
            self.cum.append(self.cum[-1] + math.hypot(x1 - x0, y1 - y0))

    @property
    def length(self):
        return self.cum[-1]

    def at(self, d):
        if d <= 0:
            return self.points[0]
        if d >= self.length:
            return self.points[-1]
        i = bisect.bisect_right(self.cum, d) - 1
        (x0, y0), (x1, y1) = self.points[i], self.points[i + 1]
        f = (d - self.cum[i]) / (self.cum[i + 1] - self.cum[i])
        return x0 + f * (x1 - x0), y0 + f * (y1 - y0)


# --- Selección del corredor --------------------------------------------------------------------

def find_runner(parsed, card):
    """Devuelve (categoría, índice en la categoría, corredor) para la tarjeta dada."""
    found = [(c, i, r) for c in parsed["classes"] for i, r in enumerate(c["runners"])
             if r.get("si_card") == card]
    if len(found) != 1:
        raise ValueError(f"la tarjeta {card} aparece {len(found)} veces en el .spl")
    cls, index, runner = found[0]
    punches = runner.get("punches", [])
    if runner.get("status") != 0:
        raise ValueError("el corredor no está clasificado (estado distinto de 0)")
    if not punches or punches[0]["code"] != ref.START or punches[-1]["code"] != ref.FINISH:
        raise ValueError("las picadas no van de salida a meta")
    times = [p["time_of_day_s"] for p in punches]
    if any(t is None or t != int(t) for t in times):
        raise ValueError("faltan picadas o no caen en segundos enteros")
    if any(b <= a for a, b in zip(times, times[1:])):
        raise ValueError("las picadas no son crecientes")
    return cls, index, runner


# --- Generación ----------------------------------------------------------------------------------

class Noise:
    """Ruido AR(1) determinista para el ritmo (factor multiplicativo alrededor de 1)."""

    def __init__(self, rng):
        self.rng, self.state = rng, 0.0

    def next(self):
        self.state = 0.85 * self.state + self.rng.gauss(0.0, 0.03)
        return min(1.15, max(0.85, 1.0 + self.state))


def leg_weights(noise, duration, stop_at=None):
    """Velocidad relativa en cada segundo del tramo: frena al llegar y al salir de las balizas
    y, si hay parada, se detiene STOP_S segundos desde `stop_at` (con rampa de 3 s)."""
    w = []
    for k in range(duration):
        shape = 1 - 0.55 * math.exp(-(k + 0.5) / 3) - 0.55 * math.exp(-(duration - k - 0.5) / 3)
        w.append(max(0.25, shape) * noise.next())
    if stop_at is not None:
        ramp = (0.7, 0.5, 0.35)
        for j, f in enumerate(ramp):
            w[stop_at - 3 + j] *= f
            w[stop_at + STOP_S + 2 - j] *= f
        for k in range(stop_at, stop_at + STOP_S):
            w[k] = 0.0
    return w


def place_controls(rng, straight):
    """Coloca la salida en el centro y cada baliza a la distancia en línea recta de su tramo."""
    pts = [(0.0, 0.0)]
    heading = rng.uniform(0, 2 * math.pi)
    for d in straight:
        x0, y0 = pts[-1]
        for attempt in range(500):
            if attempt < 400:
                h = heading + rng.uniform(-2.4, 2.4)
            else:  # demasiado lejos: hacia el centro
                h = math.atan2(-x0, -y0) + rng.uniform(-0.6, 0.6)
            x, y = x0 + d * math.sin(h), y0 + d * math.cos(h)
            if math.hypot(x, y) <= AREA_RADIUS_M and all(
                    math.hypot(x - px, y - py) >= MIN_CONTROL_SEPARATION_M for px, py in pts):
                break
        else:
            raise ValueError("no se pudo colocar una baliza")
        pts.append((x, y))
        heading = h
    return pts


def leg_path(rng, a, b, ratio, detour):
    """Camino del tramo con la longitud ratio x línea recta: un vértice (calles) o una U (rodeo)."""
    (ax, ay), (bx, by) = a, b
    dist = math.hypot(bx - ax, by - ay)
    nx, ny = -(by - ay) / dist, (bx - ax) / dist
    if detour:
        h = (ratio - 1) * dist / 2
        mx, my = (ax + bx) / 2, (ay + by) / 2
        side = 1 if math.hypot(mx + nx * h, my + ny * h) <= math.hypot(mx - nx * h, my - ny * h) \
            else -1
        return [a, (ax + side * nx * h, ay + side * ny * h),
                (bx + side * nx * h, by + side * ny * h), b]
    side = rng.choice((-1, 1))
    if ratio <= 1.0 + 1e-9:
        return [a, b]
    h = dist / 2 * math.sqrt(ratio ** 2 - 1)
    return [a, ((ax + bx) / 2 + side * nx * h, (ay + by) / 2 + side * ny * h), b]


def generate(parsed, card, seed=DEFAULT_SEED, detour_leg=None):
    """Genera el track y la verdad. `detour_leg` es el número de tramo (desde 1)."""
    cls, index, runner = find_runner(parsed, card)
    date = parsed["event"]["date"]
    punches = runner["punches"]
    local_s = [int(p["time_of_day_s"]) for p in punches]
    splits = [b - a for a, b in zip(local_s, local_s[1:])]
    if detour_leg is None:
        detour_leg = splits.index(max(splits)) + 1
    li = detour_leg - 1
    if not 0 <= li < len(splits):
        raise ValueError(f"el tramo {detour_leg} no existe")
    min_split = STOP_S + 2 * (PUNCH_MARGIN_S + 3) + 20
    if splits[li] < min_split:
        raise ValueError(f"el tramo del rodeo dura {splits[li]} s; hacen falta {min_split}")
    stop_at = round(splits[li] * 0.45)

    rng = random.Random(seed)
    noise = Noise(rng)
    legs = []
    for i, duration in enumerate(splits):
        detour = i == li
        w = leg_weights(noise, duration, stop_at if detour else None)
        speed = DETOUR_RUN_SPEED if detour else RUN_SPEED * rng.uniform(0.94, 1.06)
        path = speed * sum(w)
        if detour:
            straight = min(MAX_LEG_M, max(MIN_LEG_M, path / 2.5))
            ratio = path / straight
            if ratio < DETOUR_MIN_RATIO:
                raise ValueError("el tramo del rodeo es demasiado corto")
        else:
            ratio = rng.uniform(1.05, 1.3)
            straight = path / ratio
            if straight > MAX_LEG_M:
                straight = MAX_LEG_M
                ratio = min(MAX_NORMAL_RATIO, path / straight)
            elif straight < MIN_LEG_M:
                straight = MIN_LEG_M
                ratio = max(1.0, path / straight)
        legs.append({"weights": w, "straight": straight, "ratio": ratio, "detour": detour})

    controls_xy = place_controls(rng, [leg["straight"] for leg in legs])

    # Muestras (segundos desde la salida, x, y).
    xy = []
    first = Polyline(leg_path(rng, controls_xy[0], controls_xy[1], legs[0]["ratio"],
                              legs[0]["detour"]))
    sx, sy = controls_xy[0]
    (ux, uy) = first.points[1]
    norm = math.hypot(ux - sx, uy - sy)
    ux, uy = (ux - sx) / norm, (uy - sy) / norm
    for k in range(PRE_START_S):
        back = (PRE_START_S - k) * WALK_SPEED
        xy.append((sx - ux * back, sy - uy * back))
    paths = []
    for i, leg in enumerate(legs):
        poly = first if i == 0 else Polyline(
            leg_path(rng, controls_xy[i], controls_xy[i + 1], leg["ratio"], leg["detour"]))
        paths.append(poly)
        total = sum(leg["weights"])
        d = 0.0
        for wk in leg["weights"]:
            xy.append(poly.at(poly.length * d / total))
            d += wk
    fx, fy = controls_xy[-1]
    (px, py) = paths[-1].points[-2]
    norm = math.hypot(fx - px, fy - py)
    ang = math.atan2((fy - py) / norm, (fx - px) / norm) + math.radians(45)
    xy.append((fx, fy))
    for k in range(1, POST_FINISH_S + 1):
        xy.append((fx + math.cos(ang) * k * WALK_SPEED, fy + math.sin(ang) * k * WALK_SPEED))

    start_utc = local_to_utc(date, local_s[0])
    t0 = start_utc - datetime.timedelta(seconds=PRE_START_S)
    samples = []
    hr = 100.0
    cum = 0.0
    for k, (x, y) in enumerate(xy):
        lat, lon = to_latlon(x, y)
        lat, lon = from_semicircles(to_semicircles(lat)), from_semicircles(to_semicircles(lon))
        step = haversine_m(samples[-1]["latlon"], (lat, lon)) if samples else WALK_SPEED
        if samples:
            cum += step
        target = 125.0 if step < 0.3 else min(188.0, 92.0 + 24.0 * step)
        hr += (target - hr) / (12.0 if target > hr else 20.0)
        if step < 0.3:
            cadence = 0
        elif step < 2.0:
            cadence = round(54 + 3 * step + rng.uniform(-1.5, 1.5))
        else:
            cadence = round(80 + 3 * step + rng.uniform(-1.5, 1.5))
        samples.append({
            "time": t0 + datetime.timedelta(seconds=k),
            "latlon": (lat, lon),
            "altitude": altitude_m(x, y) + rng.gauss(0.0, 0.15),
            "heart_rate": round(hr + rng.gauss(0.0, 1.0)),
            "cadence": cadence,
            "distance": cum,
            "speed": step,
        })

    offset = start_utc.astimezone(ZoneInfo(TIMEZONE)).utcoffset()
    controls = []
    for p, s in zip(punches, local_s):
        k = PRE_START_S + s - local_s[0]
        controls.append({
            "code": p["code"],
            "lat": round(samples[k]["latlon"][0], 7),
            "lon": round(samples[k]["latlon"][1], 7),
            "punch_utc": iso_utc(samples[k]["time"]),
            "punch_local": (local_to_utc(date, s).astimezone(ZoneInfo(TIMEZONE)).isoformat()),
            "sample_index": k,
        })
    leg_truth = []
    for i, leg in enumerate(legs):
        a, b = controls[i]["sample_index"], controls[i + 1]["sample_index"]
        path_m = samples[b]["distance"] - samples[a]["distance"]
        straight_m = haversine_m(samples[a]["latlon"], samples[b]["latlon"])
        leg_truth.append({
            "leg": i + 1, "from": controls[i]["code"], "to": controls[i + 1]["code"],
            "split_s": splits[i], "path_m": round(path_m, 2),
            "straight_m": round(straight_m, 2), "ratio": round(path_m / straight_m, 3),
        })
    stop_k = controls[li]["sample_index"] + stop_at
    truth = {
        "description": "Track sintético generado por tools/fit_sintetico.py: posiciones "
                       "inventadas, tiempos de picada reales del fixture anonimizado.",
        "source_spl": None,
        "event_date_local": date,
        "timezone": TIMEZONE,
        "utc_offset_s": int(offset.total_seconds()),
        "utc_offset": _fmt_offset(offset),
        "runner": {
            "class_id": cls["id"], "class_name": cls.get("name"), "index_in_class": index,
            "spl_id": runner["id"], "bib": runner.get("bib"), "si_card": runner.get("si_card"),
            "given": runner.get("given"), "family": runner.get("family"),
            "status": runner.get("status"), "place": runner.get("place"),
        },
        "track": {
            "start_utc": iso_utc(samples[0]["time"]),
            "end_utc": iso_utc(samples[-1]["time"]),
            "records": len(samples),
            "sample_interval_s": 1,
            "pre_start_s": PRE_START_S,
            "post_finish_s": POST_FINISH_S,
            "total_distance_m": round(samples[-1]["distance"], 2),
        },
        "controls": controls,
        "legs": leg_truth,
        "detour": dict(leg_truth[li]),
        "stop": {
            "leg": li + 1,
            "start_utc": iso_utc(samples[stop_k]["time"]),
            "end_utc": iso_utc(samples[stop_k + STOP_S]["time"]),
            "duration_s": STOP_S,
            "seconds_after_punch": stop_at,
            "seconds_before_next_punch": splits[li] - stop_at - STOP_S,
            "lat": round(samples[stop_k]["latlon"][0], 7),
            "lon": round(samples[stop_k]["latlon"][1], 7),
        },
        "generation": {
            "seed": seed,
            "center": list(CENTER),
            "area_radius_m": AREA_RADIUS_M,
            "straight_leg_range_m": [MIN_LEG_M, MAX_LEG_M],
            "distance_method": f"haversine, R = {EARTH_RADIUS_M} m",
            "fit_units": {
                "position": "semicírculos",
                "altitude": "m (escala 5, offset 500)",
                "distance": "m (escala 100), acumulada entre muestras",
                "speed": "m/s (escala 1000), distancia a la muestra anterior",
                "cadence": "rpm de un pie (pasos/min = 2 x cadencia)",
                "heart_rate": "ppm",
            },
        },
    }
    return samples, truth


def _fmt_offset(offset):
    minutes = int(offset.total_seconds()) // 60
    sign = "+" if minutes >= 0 else "-"
    return f"{sign}{abs(minutes) // 60:02d}:{abs(minutes) % 60:02d}"


# --- Codificador FIT mínimo --------------------------------------------------------------------

CRC_TABLE = (0x0000, 0xCC01, 0xD801, 0x1400, 0xF001, 0x3C00, 0x2800, 0xE401,
             0xA001, 0x6C00, 0x7800, 0xB401, 0x5000, 0x9C01, 0x8801, 0x4400)


def crc16(data, crc=0):
    """CRC de 16 bits del protocolo FIT (CRC-16/ARC)."""
    for byte in data:
        tmp = CRC_TABLE[crc & 0xF]
        crc = ((crc >> 4) & 0x0FFF) ^ tmp ^ CRC_TABLE[byte & 0xF]
        tmp = CRC_TABLE[crc & 0xF]
        crc = ((crc >> 4) & 0x0FFF) ^ tmp ^ CRC_TABLE[(byte >> 4) & 0xF]
    return crc


# Tipo base FIT -> (código, formato struct)
BASE_TYPES = {
    "enum": (0x00, "B"), "uint8": (0x02, "B"), "uint16": (0x84, "H"),
    "sint32": (0x85, "i"), "uint32": (0x86, "I"), "uint32z": (0x8C, "I"),
}

# Mensajes: (número global, [(número de campo, nombre, tipo base)])
FILE_ID = (0, [(0, "type", "enum"), (1, "manufacturer", "uint16"), (2, "product", "uint16"),
               (3, "serial_number", "uint32z"), (4, "time_created", "uint32")])
EVENT = (21, [(253, "timestamp", "uint32"), (0, "event", "enum"), (1, "event_type", "enum")])
RECORD = (20, [(253, "timestamp", "uint32"), (0, "position_lat", "sint32"),
               (1, "position_long", "sint32"), (2, "altitude", "uint16"),
               (3, "heart_rate", "uint8"), (4, "cadence", "uint8"),
               (5, "distance", "uint32"), (6, "speed", "uint16")])
LAP = (19, [(253, "timestamp", "uint32"), (2, "start_time", "uint32"),
            (7, "total_elapsed_time", "uint32"), (8, "total_timer_time", "uint32"),
            (9, "total_distance", "uint32"), (0, "event", "enum"), (1, "event_type", "enum")])
SESSION = (18, [(253, "timestamp", "uint32"), (2, "start_time", "uint32"),
                (7, "total_elapsed_time", "uint32"), (8, "total_timer_time", "uint32"),
                (9, "total_distance", "uint32"), (5, "sport", "enum"),
                (6, "sub_sport", "enum"), (0, "event", "enum"), (1, "event_type", "enum"),
                (25, "first_lap_index", "uint16"), (26, "num_laps", "uint16")])
ACTIVITY = (34, [(253, "timestamp", "uint32"), (0, "total_timer_time", "uint32"),
                 (1, "num_sessions", "uint16"), (2, "type", "enum"), (3, "event", "enum"),
                 (4, "event_type", "enum"), (5, "local_timestamp", "uint32")])

MANUFACTURER_DEVELOPMENT = 255
EVENT_TIMER, EVENT_SESSION, EVENT_LAP, EVENT_ACTIVITY = 0, 8, 9, 26
EVENT_TYPE_START, EVENT_TYPE_STOP, EVENT_TYPE_STOP_ALL = 0, 1, 4
FILE_ACTIVITY, SPORT_RUNNING = 4, 1
PROTOCOL_VERSION, PROFILE_VERSION = 0x20, 2140


class FitWriter:
    def __init__(self):
        self.data = bytearray()
        self.local = {}

    def write(self, message, **values):
        global_num, fields = message
        if global_num not in self.local:
            local = self.local[global_num] = len(self.local)
            self.data += struct.pack("<BBBHB", 0x40 | local, 0, 0, global_num, len(fields))
            for num, _, base in fields:
                code, fmt = BASE_TYPES[base]
                self.data += struct.pack("<BBB", num, struct.calcsize(fmt), code)
        self.data.append(self.local[global_num])
        for _, name, base in fields:
            self.data += struct.pack("<" + BASE_TYPES[base][1], values[name])

    def finish(self):
        header = struct.pack("<BBHI4s", 14, PROTOCOL_VERSION, PROFILE_VERSION, len(self.data),
                             b".FIT")
        header += struct.pack("<H", crc16(header))
        body = header + bytes(self.data)
        return body + struct.pack("<H", crc16(body))


def encode_fit(samples, utc_offset_s):
    first, last = fit_timestamp(samples[0]["time"]), fit_timestamp(samples[-1]["time"])
    elapsed_ms = (last - first) * 1000
    distance_cm = round(samples[-1]["distance"] * 100)
    w = FitWriter()
    w.write(FILE_ID, type=FILE_ACTIVITY, manufacturer=MANUFACTURER_DEVELOPMENT, product=1,
            serial_number=4, time_created=first)
    w.write(EVENT, timestamp=first, event=EVENT_TIMER, event_type=EVENT_TYPE_START)
    for s in samples:
        w.write(RECORD, timestamp=fit_timestamp(s["time"]),
                position_lat=to_semicircles(s["latlon"][0]),
                position_long=to_semicircles(s["latlon"][1]),
                altitude=round((s["altitude"] + 500) * 5), heart_rate=s["heart_rate"],
                cadence=s["cadence"], distance=round(s["distance"] * 100),
                speed=round(s["speed"] * 1000))
    w.write(EVENT, timestamp=last, event=EVENT_TIMER, event_type=EVENT_TYPE_STOP_ALL)
    w.write(LAP, timestamp=last, start_time=first, total_elapsed_time=elapsed_ms,
            total_timer_time=elapsed_ms, total_distance=distance_cm, event=EVENT_LAP,
            event_type=EVENT_TYPE_STOP)
    w.write(SESSION, timestamp=last, start_time=first, total_elapsed_time=elapsed_ms,
            total_timer_time=elapsed_ms, total_distance=distance_cm, sport=SPORT_RUNNING,
            sub_sport=0, event=EVENT_SESSION, event_type=EVENT_TYPE_STOP, first_lap_index=0,
            num_laps=1)
    w.write(ACTIVITY, timestamp=last, total_timer_time=elapsed_ms, num_sessions=1, type=0,
            event=EVENT_ACTIVITY, event_type=EVENT_TYPE_STOP,
            local_timestamp=last + utc_offset_s)
    return w.finish()


# --- CLI -----------------------------------------------------------------------------------------

def build(spl_path, fit_path, card, seed=DEFAULT_SEED, detour_leg=None):
    """Lee el .spl y devuelve (bytes del FIT, verdad)."""
    with open(spl_path, "rb") as f:
        parsed = ref.parse(f.read())
    samples, truth = generate(parsed, card, seed, detour_leg)
    spl_path, fit_path = spl_path.replace(os.sep, "/"), fit_path.replace(os.sep, "/")
    truth["source_spl"] = spl_path
    command = f"python3 tools/fit_sintetico.py {spl_path} {fit_path} --card {card} --seed {seed}"
    if detour_leg is not None:
        command += f" --detour-leg {detour_leg}"
    truth["generation"]["command"] = command
    return encode_fit(samples, truth["utc_offset_s"]), truth


def truth_path(fit_path):
    return os.path.splitext(fit_path)[0] + ".truth.json"


def dump_truth(truth):
    return json.dumps(truth, ensure_ascii=False, indent=1) + "\n"


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("spl", help=".spl de entrada (anonimizado)")
    parser.add_argument("fit", help="FIT de salida; la verdad va al lado en .truth.json")
    parser.add_argument("--card", type=int, required=True, help="tarjeta SI del corredor")
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument("--detour-leg", type=int, help="tramo del rodeo (desde 1); por "
                                                       "defecto, el de split más largo")
    args = parser.parse_args(argv[1:])
    fit, truth = build(args.spl, args.fit, args.card, args.seed, args.detour_leg)
    with open(args.fit, "wb") as f:
        f.write(fit)
    with open(truth_path(args.fit), "w", encoding="utf-8") as f:
        f.write(dump_truth(truth))
    d = truth["detour"]
    print(f"{args.fit}: {truth['track']['records']} records, "
          f"{truth['track']['start_utc']} - {truth['track']['end_utc']} "
          f"(UTC{truth['utc_offset']}), {len(truth['legs'])} tramos; rodeo en el tramo "
          f"{d['leg']} (ratio {d['ratio']}), parada de {STOP_S} s.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
